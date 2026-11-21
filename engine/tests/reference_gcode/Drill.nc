( WHAM_CAM 0.4.0 )
( Project: Drill   reference )
( Post-processor: Generic   Machine: Default )
( T1: 1mm 2F Spiral End Mill, 20mm flute, D1 )
G21 G90 G17
G0 Z3
( Drill1: Drill [Standard], T1 D1, stock 0, target -12 )
( T1: 1mm 2F Spiral End Mill, 20mm flute )
T1 M6
S1000 M3
G4 P2
G98 G81 X34 Y22 Z-12 R3 F300
Y8
X53
Y22
G80
M5
M30
