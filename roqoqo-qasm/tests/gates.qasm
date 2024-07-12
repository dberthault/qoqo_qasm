OPENQASM 2.0;
creg c[2];
qreg q[2];

rz(0.2) q[0];
ry(0.3) q[1];
rx(2.1) q[2];
h q[0];
x q[2];
y q[1];
z q[0];
s q[0];
t q[1];
p(0.6) q[2];
sx q[1];
sxdg q[0];
cx q[0],q[1];
rxx(3.14159265359) q[1],q[2];
rxx(0.7) q[0],q[2];
cy q[0],q[1];
cz q[1],q[2];
cp(0.5) q[0],q[2];
crx(0.8) q[1],q[2];
crxy(0.4,0.2) q[0],q[1];
swap q[1],q[2];
iswap q[0],q[1];
siswap q[1],q[2];
siswapdg q[0],q[2];
fswap q[0],q[1];
fsim(0.1,0.2,0.3) q[1],q[2];
qsim(1.0,1.1,1.2) q[0],q[2];
pmint(0.8) q[1],q[2];
gvnsrot(0.2,0.9) q[1],q[2];
gvnsrotle(0,0.8) q[0],q[2];
xy(0.5) q[1],q[2];
spintint(0.5,0.6,0.7) q[0],q[2];
rxy(0.3,0.9) q[0];
pscz(0.3) q[0],q[2];
pscp(1,1.9) q[0],q[1];
ccx q[0],q[2],q[1];
ccz q[2],q[1],q[0];
ccp(0.3) q[1],q[0],q[2];
reset q[1];

measure q[0] -> c[0];