( WHAM_CAM 0.6.0 )
( Project: Untitled   2026-11-22 20:45 )
( Post-processor: Generic   Machine: Default )
( T2: 6mm 2F Upcut End Mill, 20mm flute, D6 )
G21 G90 G17
G0 Z3
( Profile1: Profile [On line], T2 D6, stock 0, target -10, depth increment 10 )
( T2: 6mm 2F Upcut End Mill, 20mm flute )
T2 M6
S10000 M3
G4 P2
G0 X0 Y0
G0 Z1
G1 Z-10 F300
G1 X100 F800
G3 X100 Y60 I-22.5 J30
G2 X0 Y60 I-50 J52.5
G0 Z3
M5
M30
