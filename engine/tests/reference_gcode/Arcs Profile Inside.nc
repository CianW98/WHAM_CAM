( WHAM_CAM 0.4.0 )
( Project: Arcs Profile Inside   reference )
( Post-processor: Generic   Machine: Default )
( T1: 6mm 2F Upcut End Mill, 20mm flute, D6 )
G21 G90 G17
G0 Z10
( Profile1: Profile [Inside], T1 D6, stock 0, target -5, depth increment 5 )
( T1: 6mm 2F Upcut End Mill, 20mm flute )
T1 M6
S1000 M3
G4 P2
G0 X3 Y3
G0 Z1
G1 Z-5 F600
G1 Y53.413 F1200
G3 X100.128 Y56.043 I47 J59.087
G2 X98.977 Y3 I-22.628 J-26.043
G1 X3
G0 Z10
M5
M30
