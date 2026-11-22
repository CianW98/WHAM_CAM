( WHAM_CAM 0.6.0 )
( Project: Untitled   2026-11-22 19:16 )
( Post-processor: Generic   Machine: Default )
( T2: 6mm 2F Upcut End Mill, 20mm flute, D6 )
G21 G90 G17
G0 Z3
( Profile1: Profile [Outside], T2 D6, stock 0, target -10, depth increment 10 )
( T2: 6mm 2F Upcut End Mill, 20mm flute )
T2 M6
S10000 M3
G4 P2
G0 X0 Y-3
G0 Z1
G1 Z-10 F300
G1 X100 F800
G3 X101.8 Y-2.4 I0 J3
G3 X101.8 Y62.4 I-24.3 J32.4
G3 X97.931 Y62.172 I-1.8 J-2.4
G2 X2.069 Y62.172 I-47.931 J50.328
G3 X-3 Y60 I-2.069 J-2.172
G1 Y0
G3 X0 Y-3 I3 J0
G0 Z3
M5
M30
