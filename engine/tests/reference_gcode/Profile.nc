( WHAM_CAM 0.4.0 )
( Project: Profile   reference )
( Post-processor: Generic   Machine: Default )
( T1: 6mm 2F Spiral End Mill, 20mm flute, D6 )
G21 G90 G17
G0 Z10
( Profile1: Profile [Outside], T1 D6, stock 0, target -5, depth increment 5 )
( T1: 6mm 2F Spiral End Mill, 20mm flute )
T1 M6
S1000 M3
G4 P2
G0 X-15.462 Y32.735
G0 Z1
G1 Z-5 F600
G3 X-36.248 Y29.677 I-6.538 J-27.735 F1200
G3 X-36.248 Y-19.677 I14.248 J-24.677
G3 X-34.753 Y-20.482 I14.248 J24.677
G1 X-41.82 Y-39.978
G3 X-38.786 Y-43.992 I2.82 J-1.022
G1 X31.214 Y-38.992
G3 X33.337 Y-37.881 I-0.214 J2.992
G1 X66.337 Y3.119
G3 X65.253 Y7.726 I-2.337 J1.881
G1 X-8.747 Y41.726
G3 X-12.82 Y40.022 I-1.253 J-2.726
G1 X-15.462 Y32.735
G0 Z10
M5
M30
