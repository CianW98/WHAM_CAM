//! The G-code read-back check (spec 7, 8): reads a written program the way a controller would and
//! makes sure it does exactly what the toolpaths say, before any file is saved. It reads only what
//! WHAM_CAM's post-processor writes; anything else is an error.
//!
//! For each operation the moves read back must match the toolpath's moves (within the rounding of
//! the number format), pass the safety checker with the operation's limits, and have the right
//! tool and spindle. The program must set mm, absolute and the XY plane before moving, move only
//! straight up or down between operations, and end at the clearance plane with the spindle off.

use crate::drill::cycle_moves;
use crate::operation::SpindleDirection;
use crate::post::{JobOperation, Program};
use crate::safety;
use crate::toolpath::{CycleKind, DrillCycle, Move, Point3};

/// Rounding allowance for coordinates written with 3 decimals (mm).
const COORD_TOL: f64 = 0.0006;
/// Arc centres are a rounded start plus rounded offsets: twice the rounding.
const CENTRE_TOL: f64 = 0.0011;
/// The largest difference between an arc's start and end radius a controller accepts (mm).
pub const ARC_RADIUS_TOL: f64 = 0.002;

/// What a line did, in program order.
#[derive(Debug, Clone, PartialEq)]
enum Event {
    /// A move, with where it started (None for the program's first sideways rapid).
    Move {
        line: usize,
        mv: Move,
        from: Option<Point3>,
    },
    /// A vertical rapid before the XY position is known (the program start).
    ZOnly {
        line: usize,
        z: f64,
    },
    ToolChange {
        line: usize,
        tool: u32,
        z: Option<f64>,
    },
    Spindle {
        line: usize,
        on: bool,
    },
    End {
        line: usize,
    },
}

/// The machine state while reading.
struct State {
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    feed: Option<f64>,
    speed: f64,
    tool: Option<u32>,
    mm: bool,
    absolute: bool,
    xy_plane: bool,
    /// The canned cycle in force (G81/G83 are modal until G80), with its initial plane.
    cycle: Option<(DrillCycle, f64)>,
    g98: bool,
}

fn word_value(word: &str, line: usize) -> Result<(char, f64), String> {
    let mut chars = word.chars();
    let letter = chars.next().unwrap_or(' ').to_ascii_uppercase();
    let value: f64 = chars
        .as_str()
        .parse()
        .map_err(|_| format!("line {}: can't read \"{word}\".", line + 1))?;
    Ok((letter, value))
}

/// Reads the program into events. `back_off` gives the peck back-off for a line (the fast plunge
/// height of the operation it belongs to).
fn read(lines: &[String], back_off: impl Fn(usize) -> f64) -> Result<Vec<Event>, String> {
    let mut s = State {
        x: None,
        y: None,
        z: None,
        feed: None,
        speed: 0.0,
        tool: None,
        mm: false,
        absolute: false,
        xy_plane: false,
        cycle: None,
        g98: false,
    };
    let mut events = Vec::new();
    for (n, raw) in lines.iter().enumerate() {
        let err = |msg: &str| Err(format!("line {}: {msg}", n + 1));
        // Comments are whole "( … )" groups.
        let mut text = String::new();
        let mut depth = 0;
        for c in raw.chars() {
            match c {
                '(' => depth += 1,
                ')' if depth > 0 => depth -= 1,
                _ if depth == 0 => text.push(c),
                _ => {}
            }
        }
        if depth != 0 {
            return err("a comment isn't closed.");
        }
        let mut gs: Vec<u32> = Vec::new();
        let mut ms: Vec<u32> = Vec::new();
        let mut words: Vec<(char, f64)> = Vec::new();
        for w in text.split_whitespace() {
            let (letter, v) = word_value(w, n)?;
            match letter {
                'G' => gs.push(v as u32),
                'M' => ms.push(v as u32),
                'X' | 'Y' | 'Z' | 'I' | 'J' | 'R' | 'Q' | 'F' | 'S' | 'T' | 'P' => {
                    words.push((letter, v))
                }
                _ => return err(&format!("unexpected word \"{w}\".")),
            }
        }
        let get = |l: char| words.iter().find(|(c, _)| *c == l).map(|(_, v)| *v);
        if let Some(f) = get('F') {
            if f < 1.0 {
                return err("a feed rate below 1 mm/min.");
            }
            s.feed = Some(f);
        }
        if let Some(sp) = get('S') {
            s.speed = sp;
        }
        let mut motion: Option<u32> = None;
        for g in &gs {
            match g {
                21 => s.mm = true,
                90 => s.absolute = true,
                17 => s.xy_plane = true,
                98 => s.g98 = true,
                80 => s.cycle = None,
                4 => {}
                0 | 1 | 2 | 3 | 81 | 83 => motion = Some(*g),
                _ => return err(&format!("unexpected code G{g}.")),
            }
        }
        // A canned cycle stays in force until G80: a line of only coordinates drills the next
        // hole with it.
        if motion.is_none()
            && let Some((c, _)) = s.cycle
            && words
                .iter()
                .any(|(l, _)| matches!(l, 'X' | 'Y' | 'Z' | 'R' | 'Q'))
        {
            motion = Some(match c.kind {
                CycleKind::Standard => 81,
                CycleKind::Peck => 83,
            });
        }
        if let Some(g) = motion {
            if !(s.mm && s.absolute && s.xy_plane) {
                return err("a move before G21 G90 G17 (mm, absolute, XY plane).");
            }
            let to_x = get('X').or(s.x);
            let to_y = get('Y').or(s.y);
            let to_z = get('Z').or(s.z);
            match g {
                0..=3 => {
                    s.cycle = None;
                    match (to_x, to_y, to_z, s.x, s.y, s.z) {
                        (Some(x), Some(y), Some(z), Some(fx), Some(fy), Some(fz)) => {
                            let from = Point3::new(fx, fy, fz);
                            let to = Point3::new(x, y, z);
                            let mv = match g {
                                0 => Move::Rapid { to },
                                1 => Move::Feed {
                                    to,
                                    feed: s.feed.ok_or_else(|| {
                                        format!("line {}: a feed move without a feed rate.", n + 1)
                                    })?,
                                },
                                _ => {
                                    let (Some(i), Some(j)) = (get('I'), get('J')) else {
                                        return err("an arc without I and J.");
                                    };
                                    Move::Arc {
                                        to,
                                        center_x: fx + i,
                                        center_y: fy + j,
                                        ccw: g == 3,
                                        feed: s.feed.ok_or_else(|| {
                                            format!("line {}: an arc without a feed rate.", n + 1)
                                        })?,
                                    }
                                }
                            };
                            events.push(Event::Move {
                                line: n,
                                mv,
                                from: Some(from),
                            });
                        }
                        (None, None, Some(z), None, None, _) if g == 0 => {
                            events.push(Event::ZOnly { line: n, z });
                        }
                        // The first sideways rapid of the program: X and Y weren't known yet.
                        (Some(x), Some(y), Some(z), None, None, Some(_)) if g == 0 => {
                            events.push(Event::Move {
                                line: n,
                                mv: Move::Rapid {
                                    to: Point3::new(x, y, z),
                                },
                                from: None,
                            });
                        }
                        _ => return err("a move before the tool's position is known."),
                    }
                    s.x = to_x;
                    s.y = to_y;
                    s.z = to_z;
                }
                _ => {
                    // G81 / G83: modal canned cycle; the first one fixes the initial plane.
                    if !s.g98 {
                        return err("a drilling cycle without G98.");
                    }
                    let kind = if g == 81 {
                        CycleKind::Standard
                    } else {
                        CycleKind::Peck
                    };
                    let (prev, initial) = match s.cycle {
                        Some((c, init)) if c.kind == kind => (Some(c), init),
                        _ => (
                            None,
                            s.z.ok_or_else(|| {
                                format!("line {}: a drilling cycle before Z is known.", n + 1)
                            })?,
                        ),
                    };
                    let pick = |l: char, before: Option<f64>| {
                        get(l).or(before).ok_or_else(|| {
                            format!("line {}: the drilling cycle has no {l}.", n + 1)
                        })
                    };
                    let cycle = DrillCycle {
                        kind,
                        x: to_x.ok_or_else(|| format!("line {}: the cycle has no X.", n + 1))?,
                        y: to_y.ok_or_else(|| format!("line {}: the cycle has no Y.", n + 1))?,
                        r: pick('R', prev.map(|c| c.r))?,
                        z: pick('Z', prev.map(|c| c.z))?,
                        q: if kind == CycleKind::Peck {
                            pick('Q', prev.map(|c| c.q))?
                        } else {
                            0.0
                        },
                        feed: s.feed.ok_or_else(|| {
                            format!("line {}: the cycle has no feed rate.", n + 1)
                        })?,
                    };
                    // As for the first rapid, X and Y may not be known yet at the program start.
                    let mut from = match (s.x, s.y, s.z) {
                        (Some(fx), Some(fy), Some(fz)) => Some(Point3::new(fx, fy, fz)),
                        (None, None, Some(_)) => None,
                        _ => return err("a drilling cycle before the tool's position is known."),
                    };
                    for mv in cycle_moves(&cycle, initial, back_off(n)) {
                        if from.is_none() && !matches!(mv, Move::Rapid { .. }) {
                            return err("a drilling cycle before the tool's position is known.");
                        }
                        events.push(Event::Move { line: n, mv, from });
                        from = Some(mv.to());
                    }
                    s.cycle = Some((cycle, initial));
                    // G98: back at the initial plane over the hole.
                    s.x = Some(cycle.x);
                    s.y = Some(cycle.y);
                    s.z = Some(initial);
                }
            }
        } else if !words
            .iter()
            .all(|(c, _)| matches!(c, 'F' | 'S' | 'T' | 'P'))
        {
            return err("coordinates without a move code.");
        }
        for m in &ms {
            match m {
                3 | 4 => {
                    if s.speed <= 0.0 {
                        return err("the spindle is started without a speed.");
                    }
                    events.push(Event::Spindle { line: n, on: true });
                }
                5 => events.push(Event::Spindle { line: n, on: false }),
                6 => {
                    let t = get('T').ok_or_else(|| format!("line {}: M6 without T.", n + 1))?;
                    s.tool = Some(t as u32);
                    events.push(Event::ToolChange {
                        line: n,
                        tool: t as u32,
                        z: s.z,
                    });
                }
                30 => events.push(Event::End { line: n }),
                _ => return err(&format!("unexpected code M{m}.")),
            }
        }
    }
    Ok(events)
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn same_point(a: Point3, b: Point3) -> bool {
    close(a.x, b.x, COORD_TOL) && close(a.y, b.y, COORD_TOL) && close(a.z, b.z, COORD_TOL)
}

/// The toolpath's moves as they can appear in the G-code: a move to where the tool already is
/// (after rounding) writes nothing, so it is left out.
fn written_moves(moves: &[Move], decimals: usize) -> Vec<Move> {
    let round = |v: f64| crate::post::number(v, decimals);
    let key = |p: Point3| (round(p.x), round(p.y), round(p.z));
    let mut out: Vec<Move> = Vec::new();
    let mut at: Option<(String, String, String)> = None;
    for m in moves {
        let k = key(m.to());
        let still = at.as_ref() == Some(&k);
        if !(still && !matches!(m, Move::Arc { .. })) {
            out.push(*m);
        }
        at = Some(k);
    }
    out
}

fn same_move(got: &Move, want: &Move) -> bool {
    if !same_point(got.to(), want.to()) {
        return false;
    }
    match (got, want) {
        (Move::Rapid { .. }, Move::Rapid { .. }) => true,
        (Move::Feed { feed: a, .. }, Move::Feed { feed: b, .. }) => close(*a, *b, 0.5),
        (
            Move::Arc {
                center_x: ax,
                center_y: ay,
                ccw: ac,
                feed: af,
                ..
            },
            Move::Arc {
                center_x: bx,
                center_y: by,
                ccw: bc,
                feed: bf,
                ..
            },
        ) => {
            ac == bc
                && close(*ax, *bx, CENTRE_TOL)
                && close(*ay, *by, CENTRE_TOL)
                && close(*af, *bf, 0.5)
        }
        _ => false,
    }
}

/// Checks a written program against the job it was written from. Returns every problem found
/// (empty = the G-code does exactly what the toolpaths say, safely).
pub fn check_program(program: &Program, job: &[JobOperation], decimals: usize) -> Vec<String> {
    let in_section = |line: usize| {
        program
            .sections
            .iter()
            .position(|s| s.moves.contains(&line))
    };
    let back_off = |line: usize| {
        in_section(line)
            .and_then(|i| job.get(i))
            .map_or(1.0, |op| op.fast_plunge_height)
    };
    let events = match read(&program.lines, back_off) {
        Ok(e) => e,
        Err(e) => return vec![e],
    };
    let mut problems = Vec::new();
    if program.sections.len() != job.len() {
        problems.push("the program doesn't have one section per operation.".into());
        return problems;
    }

    // Each operation: its moves, tool and spindle.
    for (i, (section, op)) in program.sections.iter().zip(job).enumerate() {
        let name = &op.name;
        let got: Vec<Move> = events
            .iter()
            .filter_map(|e| match e {
                Event::Move { line, mv, .. } if section.moves.contains(line) => Some(*mv),
                _ => None,
            })
            .collect();
        let want = written_moves(&op.toolpath.moves, decimals);
        let mut got_cmp = got.clone();
        if !op.toolpath.cycles.is_empty() {
            got_cmp = written_moves(&got, decimals);
        }
        if got_cmp.len() != want.len() {
            problems.push(format!(
                "{name}: the G-code has {} moves, the toolpath {}.",
                got_cmp.len(),
                want.len()
            ));
        } else if let Some(k) = (0..want.len()).find(|&k| !same_move(&got_cmp[k], &want[k])) {
            problems.push(format!(
                "{name}: move {} in the G-code isn't the toolpath's move.",
                k + 1
            ));
        }
        for p in safety::check(&got, &op.toolpath.limits) {
            problems.push(format!("{name}: {p}"));
        }
        // Arcs: start and end radius must agree, as controllers require.
        for e in &events {
            if let Event::Move {
                line,
                mv:
                    Move::Arc {
                        to,
                        center_x,
                        center_y,
                        ..
                    },
                from: Some(from),
            } = e
                && section.moves.contains(line)
            {
                let r0 = (from.x - center_x).hypot(from.y - center_y);
                let r1 = (to.x - center_x).hypot(to.y - center_y);
                if (r0 - r1).abs() > ARC_RADIUS_TOL {
                    problems.push(format!(
                        "{name}: line {}: the arc's start and end radius differ by {:.4} mm.",
                        line + 1,
                        (r0 - r1).abs()
                    ));
                }
            }
        }
        // The tool and spindle in force when its moves start.
        let start = section.moves.start;
        let mut tool = None;
        let mut spindle = false;
        for e in &events {
            match e {
                Event::ToolChange { line, tool: t, z } if *line < start => {
                    tool = Some(*t);
                    let clear = job
                        .iter()
                        .zip(&program.sections)
                        .find(|(_, s)| s.moves.start > *line)
                        .map_or(f64::NEG_INFINITY, |(o, _)| {
                            o.toolpath.limits.clearance_plane
                        });
                    if z.is_none_or(|z| z < clear - COORD_TOL) {
                        problems.push(format!(
                            "line {}: a tool change below the clearance plane.",
                            line + 1
                        ));
                    }
                }
                Event::Spindle { line, on } if *line < start => spindle = *on,
                _ => {}
            }
        }
        if tool != Some(op.tool_number) {
            problems.push(format!(
                "{name}: the tool in the G-code isn't T{}.",
                op.tool_number
            ));
        }
        if op.spindle != SpindleDirection::Off && !spindle {
            problems.push(format!("{name}: the spindle isn't on before cutting."));
        }
        let _ = i;
    }

    // Between operations: only straight up or down, never below the next clearance plane.
    for e in &events {
        let (line, ok) = match e {
            Event::ZOnly { line, .. } => (*line, true),
            Event::Move { line, mv, from } if in_section(*line).is_none() => {
                let to = mv.to();
                let straight = from
                    .is_some_and(|f| close(f.x, to.x, COORD_TOL) && close(f.y, to.y, COORD_TOL));
                let next_clear = job
                    .iter()
                    .zip(&program.sections)
                    .find(|(_, s)| s.moves.start > *line)
                    .map_or(f64::NEG_INFINITY, |(o, _)| {
                        o.toolpath.limits.clearance_plane
                    });
                (
                    *line,
                    matches!(mv, Move::Rapid { .. }) && straight && to.z >= next_clear - COORD_TOL,
                )
            }
            _ => continue,
        };
        if !ok {
            problems.push(format!(
                "line {}: a move between operations that isn't straight up or down to a clearance plane.",
                line + 1
            ));
        }
    }

    // The end: spindle off, then M30, at the last clearance plane.
    let last_clear = job.last().map(|o| o.toolpath.limits.clearance_plane);
    let ends = matches!(events.last(), Some(Event::End { .. }));
    let spindle_off = events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Spindle { on, .. } => Some(!on),
            _ => None,
        })
        .unwrap_or(true);
    let last_z = events.iter().rev().find_map(|e| match e {
        Event::Move { mv, .. } => Some(mv.to().z),
        Event::ZOnly { z, .. } => Some(*z),
        _ => None,
    });
    if !ends {
        problems.push("the program doesn't end with M30.".into());
    }
    if !spindle_off {
        problems.push("the spindle isn't turned off at the end.".into());
    }
    if let (Some(z), Some(clear)) = (last_z, last_clear)
        && !close(z, clear, COORD_TOL)
    {
        problems.push("the program doesn't end at the clearance plane.".into());
    }
    problems
}
