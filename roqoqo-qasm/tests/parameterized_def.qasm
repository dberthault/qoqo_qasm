OPENQASM 2.0;
include "qelib1.inc";
gate r(param0,param1) q0 { u(param0,-pi/2 + param1,pi/2 - param1) q0; }
qreg q[20];
creg fold0_ham0_trotter2_twirl0_basisX_trex0_0[2];
r(pi/2,-1.055552498497585) q[1];
r(pi/10,0) q[4];
cz q[4],q[1];
measure q[4] -> fold0_ham0_trotter2_twirl0_basisX_trex0_0[0];
measure q[1] -> fold0_ham0_trotter2_twirl0_basisX_trex0_0[1];
