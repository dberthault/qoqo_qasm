// Copyright © 2021-2023 HQS Quantum Simulations GmbH. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the
// License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either
// express or implied. See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
    call_operation, gate_definition, VariableGatherer, ALLOWED_OPERATIONS,
    NO_DEFINITION_REQUIRED_OPERATIONS,
};
use qoqo_calculator::CalculatorFloat;
use roqoqo::operations::*;
use roqoqo::{Circuit, RoqoqoBackendError};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

/// Checks for new declarations in the circuit.
fn process_operation_circuit<'a>(
    circuit: impl Iterator<Item = &'a Operation>,
    qasm_version: QasmVersion,
    already_seen_declarations: &mut Vec<String>,
    declarations: &mut String,
) -> Result<(), RoqoqoBackendError> {
    for operation in circuit {
        if !already_seen_declarations.contains(&operation.hqslang().to_string()) {
            already_seen_declarations.push(operation.hqslang().to_string());
            declarations.push_str(&gate_definition(operation, qasm_version)?);
            if !declarations.is_empty() {
                declarations.push('\n');
            }
        }
    }
    Ok(())
}

/// QASM backend to qoqo
///
/// This backend to roqoqo produces QASM output which can be exported.
///
/// This backend takes a roqoqo circuit and returns a QASM String or writes a QASM file
/// containing the translated circuit. The circuit itself is translated using the roqoqo-qasm
/// interface. In this backend, the initialization sets up the relevant parameters and the run
/// function calls the QASM interface and writes the QASM file, which is saved to be used by the
/// user on whatever platform they see fit. QASM input is widely supported on various quantum
/// computing platforms.
///
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backend {
    /// Name of the qubit_register assigned to the roqoqo qubits.
    ///
    /// roqoqo uses as unified address-space for qubits.
    /// There are no separate quantum registers and all qubits are addressed with `usize` addresses.
    /// When translating to QASM which uses explicitely declared qubit registers a name for the qubit
    /// register needs to be chosen.
    qubit_register_name: String,
    /// Which version of OpenQASM (2.0 or 3.0) to use
    qasm_version: QasmVersion,
}

impl Backend {
    /// Creates new QASM backend.
    ///
    /// # Arguments
    ///
    /// * `qubit_register_name` - The number of qubits in the backend.
    /// * `qasm_version` - The version of OpenQASM (2.0 or 3.0) to use.
    pub fn new(
        qubit_register_name: Option<String>,
        qasm_version: Option<String>,
    ) -> Result<Self, RoqoqoBackendError> {
        let qubit_reg = match qubit_register_name {
            None => "q".to_string(),
            Some(s) => s,
        };
        let qasm_v = match qasm_version {
            None => QasmVersion::V2point0(Qasm2Dialect::Vanilla),
            Some(v) => QasmVersion::from_str(v.as_str())?,
        };

        Ok(Self {
            qubit_register_name: qubit_reg,
            qasm_version: qasm_v,
        })
    }

    /// Translates an iterator over operations to a valid QASM string.
    ///
    ///
    /// # Arguments
    ///
    /// * `circuit` - The iterator over [roqoqo::Operation] items that is translated
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The valid QASM string
    /// * `RoqoqoBackendError::OperationNotInBackend` - An operation is not available on the backend
    pub fn circuit_iterator_to_qasm_str<'a>(
        &self,
        circuit: impl Iterator<Item = &'a Operation>,
    ) -> Result<String, RoqoqoBackendError> {
        // Initializing data structures
        let mut definitions: String = "".to_string();
        let mut data: String = "".to_string();
        let mut number_qubits_required: usize = 0;
        let mut already_seen_definitions: Vec<String> = vec![
            "RotateX".to_string(),
            "RotateY".to_string(),
            "RotateZ".to_string(),
            "CNOT".to_string(),
        ];
        let mut variable_gatherer = VariableGatherer::new();

        // Appending QASM version
        let mut qasm_string = String::from("OPENQASM ");
        match self.qasm_version {
            QasmVersion::V2point0(_) => qasm_string.push_str("2.0;\n\n"),
            QasmVersion::V3point0(_) => qasm_string.push_str("3.0;\n\n"),
        }

        // Appending definitions that are always needed (some depend on QASM version)
        definitions.push_str("gate u3(theta,phi,lambda) q { U(theta,phi,lambda) q; }\n");
        definitions.push_str("gate u2(phi,lambda) q { U(pi/2,phi,lambda) q; }\n");
        definitions.push_str("gate u1(lambda) q { U(0,0,lambda) q; }\n");
        definitions.push_str(&gate_definition(
            &Operation::from(RotateX::new(0, CalculatorFloat::from(0.0))),
            self.qasm_version,
        )?);
        definitions.push('\n');
        definitions.push_str(&gate_definition(
            &Operation::from(RotateY::new(0, CalculatorFloat::from(0.0))),
            self.qasm_version,
        )?);
        definitions.push('\n');
        definitions.push_str(&gate_definition(
            &Operation::from(RotateZ::new(0, CalculatorFloat::from(0.0))),
            self.qasm_version,
        )?);
        definitions.push('\n');
        definitions.push_str(&gate_definition(
            &Operation::from(CNOT::new(0, 1)),
            self.qasm_version,
        )?);
        definitions.push_str("\n\n");

        // Main loop over the circuit
        for op in circuit {
            // Taking note of the maximum number of qubits involved in the circuit for registers definition
            if let InvolvedQubits::Set(involved_qubits) = op.involved_qubits() {
                number_qubits_required =
                    number_qubits_required.max(match involved_qubits.iter().max() {
                        None => 0,
                        Some(n) => *n,
                    })
            }

            // Appending gate definition if not already seen before
            if !already_seen_definitions.contains(&op.hqslang().to_string()) {
                let mut continue_process = false;
                if let Operation::GateDefinition(gate_definition) = op {
                    if !already_seen_definitions.contains(gate_definition.name()) {
                        already_seen_definitions.push(gate_definition.name().to_owned());
                        continue_process = true;
                    }
                } else {
                    already_seen_definitions.push(op.hqslang().to_string());
                    continue_process = true;
                }

                if continue_process {
                    match op {
                        Operation::GateDefinition(gate_definition) => process_operation_circuit(
                            gate_definition.circuit().iter(),
                            self.qasm_version,
                            &mut already_seen_definitions,
                            &mut definitions,
                        )?,
                        Operation::PragmaConditional(pragma_conditional) => {
                            process_operation_circuit(
                                pragma_conditional.circuit().iter(),
                                self.qasm_version,
                                &mut already_seen_definitions,
                                &mut definitions,
                            )?
                        }
                        Operation::PragmaLoop(pragma_loop) => process_operation_circuit(
                            pragma_loop.circuit().iter(),
                            self.qasm_version,
                            &mut already_seen_definitions,
                            &mut definitions,
                        )?,
                        _ => {}
                    }
                    definitions.push_str(&gate_definition(op, self.qasm_version)?);
                    if !definitions.is_empty()
                        && !NO_DEFINITION_REQUIRED_OPERATIONS.contains(&op.hqslang())
                    {
                        definitions.push('\n');
                    }
                }
            }
            // Appending operation QASM instruction
            data.push_str(&call_operation(
                op,
                &self.qubit_register_name,
                self.qasm_version,
                &mut Some(&mut variable_gatherer),
            )?);

            if !data.is_empty() && !ALLOWED_OPERATIONS.contains(&op.hqslang()) {
                data.push('\n');
            }
        }

        // Building the final string: QASM version + definitions + parameters + registers + circuit data
        match self.qasm_version {
            QasmVersion::V3point0(Qasm3Dialect::Braket) => {}
            QasmVersion::V2point0(Qasm2Dialect::Qulacs) => {
                qasm_string.push_str("include \"qelib1.inc\";\n\n")
            }
            _ => qasm_string.push_str(definitions.as_str()),
        };

        if let QasmVersion::V3point0(_) = self.qasm_version {
            if !variable_gatherer.variables.is_empty() {
                qasm_string.push('\n');
                for var in &variable_gatherer.variables {
                    qasm_string.push_str(format!("input angle[32] {};\n", var).as_str());
                }
                qasm_string.push('\n');
            }
        }
        match self.qasm_version {
            QasmVersion::V2point0(_) => qasm_string.push_str(
                format!(
                    "\nqreg {}[{}];\n\n",
                    self.qubit_register_name,
                    number_qubits_required + 1,
                )
                .as_str(),
            ),
            QasmVersion::V3point0(_) => qasm_string.push_str(
                format!(
                    "\nqubit[{}] {};\n\n",
                    number_qubits_required + 1,
                    self.qubit_register_name,
                )
                .as_str(),
            ),
        }
        qasm_string.push_str(data.as_str());

        Ok(qasm_string)
    }

    /// Translates an iterator over operations to a QASM file.
    ///
    /// # Arguments
    ///
    /// * `circuit` - The iterator over [roqoqo::Operation] items that is translated
    /// * `folder_name` - The name of the folder that is prepended to all filenames.
    /// * `filename` - The name of the file the QASM text is saved to.
    /// * `overwrite` - Whether to overwrite file if it already exists.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - The qasm file was correctly written
    /// * `RoqoqoBackendError::FileAlreadyExists` - The file at this location already exists
    pub fn circuit_iterator_to_qasm_file<'a>(
        &self,
        circuit: impl Iterator<Item = &'a Operation>,
        folder_name: &Path,
        filename: &Path,
        overwrite: bool,
    ) -> Result<(), RoqoqoBackendError> {
        let data: String = self.circuit_iterator_to_qasm_str(circuit)?;

        let output_path: PathBuf = folder_name.join(filename.with_extension("qasm"));
        if output_path.is_file() && !overwrite {
            return Err(RoqoqoBackendError::FileAlreadyExists {
                path: output_path.to_str().unwrap().to_string(),
            });
        } else {
            let f = File::create(output_path).expect("Unable to create file");
            let mut f = BufWriter::new(f);
            f.write_all(data.as_bytes()).expect("Unable to write file")
        }

        Ok(())
    }

    /// Translates a Circuit to a valid QASM string.
    ///
    ///
    /// # Arguments
    ///
    /// * `circuit` - The Circuit items that is translated
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The valid QASM string
    /// * `RoqoqoBackendError::OperationNotInBackend` - An operation is not available on the backend
    pub fn circuit_to_qasm_str(&self, circuit: &Circuit) -> Result<String, RoqoqoBackendError> {
        self.circuit_iterator_to_qasm_str(circuit.iter())
    }

    /// Translates a Circuit to a QASM file.
    ///
    /// # Arguments
    ///
    /// * `circuit` - The Circuit that is translated
    /// * `folder_name` - The name of the folder that is prepended to all filenames.
    /// * `filename` - The name of the file the QASM text is saved to.
    /// * `overwrite` - Whether to overwrite file if it already exists.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - The qasm file was correctly written
    /// * `RoqoqoBackendError::FileAlreadyExists` - The file at this location already exists
    pub fn circuit_to_qasm_file(
        &self,
        circuit: &Circuit,
        folder_name: &Path,
        filename: &Path,
        overwrite: bool,
    ) -> Result<(), RoqoqoBackendError> {
        self.circuit_iterator_to_qasm_file(circuit.iter(), folder_name, filename, overwrite)
    }

    /// Translates a QASM file into a qoqo Circuit instance.
    ///
    /// # Arguments
    ///
    /// * `file` - The '.qasm' file to translate.
    ///
    /// # Returns
    ///
    /// * `Ok(Circuit)` - The translated qoqo Circuit.
    /// * `RoqoqoBackendError::GenericError` - Error encountered while parsing.
    pub fn file_to_circuit(&self, file: File) -> Result<Circuit, RoqoqoBackendError> {
        crate::file_to_circuit(file)
    }

    /// Translates a QASM string into a qoqo Circuit instance.
    ///
    /// # Arguments
    ///
    /// * `input` - The QASM string to translate.
    ///
    /// # Returns
    ///
    /// * `Ok(Circuit)` - The translated qoqo Circuit.
    /// * `RoqoqoBackendError::GenericError` - Error encountered while parsing.
    pub fn string_to_circuit(&self, input: &str) -> Result<Circuit, RoqoqoBackendError> {
        crate::string_to_circuit(input)
    }
}

/// Enum for setting the version of OpenQASM used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QasmVersion {
    /// OpenQASM 2.0
    V2point0(Qasm2Dialect),
    /// OpenQASM 3.0
    V3point0(Qasm3Dialect),
}

/// Enum for setting the version of OpenQASM used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qasm2Dialect {
    /// Vanilla OpenQasm 2.0
    Vanilla,
    /// Without gate definitions
    Qulacs,
}

/// Enum for setting the version of OpenQASM used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qasm3Dialect {
    /// No Pragma operations
    Vanilla,
    /// With Pragma operations
    Roqoqo,
    /// With Braket's Pragma operations
    Braket,
}

impl FromStr for QasmVersion {
    type Err = RoqoqoBackendError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "2.0" | "2.0Vanilla" => Ok(QasmVersion::V2point0(Qasm2Dialect::Vanilla)),
            "2.0Qulacs" => Ok(QasmVersion::V2point0(Qasm2Dialect::Qulacs)),
            "3.0Roqoqo" => Ok(QasmVersion::V3point0(Qasm3Dialect::Roqoqo)),
            "3.0Braket" => Ok(QasmVersion::V3point0(Qasm3Dialect::Braket)),
            "3.0Vanilla" => Ok(QasmVersion::V3point0(Qasm3Dialect::Vanilla)),
            "3.0" => Ok(QasmVersion::V3point0(Qasm3Dialect::Vanilla)),
            _ => Err(RoqoqoBackendError::GenericError {
                msg: format!("Version for OpenQASM used is neither 2.0 nor 3.0: {}", s),
            }),
        }
    }
}
