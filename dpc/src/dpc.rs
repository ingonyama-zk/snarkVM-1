// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

use crate::prelude::*;
use snarkvm_algorithms::prelude::*;

use anyhow::Result;
use rand::{CryptoRng, Rng};
use std::marker::PhantomData;

pub struct DPC<N: Network>(PhantomData<N>);

impl<N: Network> DPCScheme<N> for DPC<N> {
    type Account = Account<N>;
    type Authorization = TransactionAuthorization<N>;
    type LedgerProof = LedgerProof<N>;
    type StateTransition = StateTransition<N>;

    /// Returns an authorization to execute a state transition.
    fn authorize<R: Rng + CryptoRng>(
        private_keys: &Vec<<Self::Account as AccountScheme>::PrivateKey>,
        transition: &Self::StateTransition,
        rng: &mut R,
    ) -> Result<Self::Authorization> {
        // Keep a cursor for the private keys.
        let mut index = 0;

        // Construct the signature message.
        let signature_message = transition.kernel().to_signature_message()?;

        // Sign the transaction kernel to authorize the transaction.
        let mut signatures = Vec::with_capacity(N::NUM_INPUT_RECORDS);
        for noop_private_key in transition.noop_private_keys().iter().take(N::NUM_INPUT_RECORDS) {
            // Fetch the correct private key.
            let private_key = match noop_private_key {
                Some(noop_private_key) => noop_private_key,
                None => {
                    let private_key = &private_keys[index];
                    index += 1;
                    private_key
                }
            };

            // Sign the signature message.
            signatures.push(private_key.sign(&signature_message, rng)?);
        }

        // Return the transaction authorization.
        Ok(TransactionAuthorization::from(transition, signatures))
    }

    /// Returns a transaction by executing an authorized state transition.
    fn execute<R: Rng + CryptoRng>(
        authorization: Self::Authorization,
        executable: &Executable<N>,
        ledger_proof: Self::LedgerProof,
        rng: &mut R,
    ) -> Result<Transaction<N>> {
        let execution_timer = start_timer!(|| "DPC::execute");

        // Construct the ledger witnesses.
        let block_hash = ledger_proof.block_hash();

        // Generate the transaction ID.
        let transaction_id = authorization.to_transaction_id()?;

        // Execute the program circuit.
        let execution = executable.execute(PublicVariables::new(transaction_id))?;

        // Compute the encrypted records.
        let (encrypted_records, encrypted_record_ids, encrypted_record_randomizers) =
            authorization.to_encrypted_records(rng)?;

        let TransactionAuthorization {
            kernel,
            input_records,
            output_records,
            signatures,
        } = authorization;

        // Construct the inner circuit public and private variables.
        let inner_public_variables = InnerPublicVariables::new(
            transaction_id,
            block_hash,
            &encrypted_record_ids,
            Some(executable.program_id()),
        )?;
        let inner_private_variables = InnerPrivateVariables::new(
            &kernel,
            input_records,
            ledger_proof,
            signatures,
            output_records.clone(),
            encrypted_record_randomizers,
            &executable,
        )?;

        // Compute the inner circuit proof.
        let inner_proof = N::InnerSNARK::prove(
            N::inner_circuit_proving_key(),
            &InnerCircuit::<N>::new(inner_public_variables.clone(), inner_private_variables),
            rng,
        )?;

        // Verify that the inner circuit proof passes.
        assert!(N::InnerSNARK::verify(
            N::inner_circuit_verifying_key(),
            &inner_public_variables,
            &inner_proof
        )?);

        // Construct the outer circuit public and private variables.
        let outer_public_variables = OuterPublicVariables::new(&inner_public_variables, *N::inner_circuit_id());
        let outer_private_variables =
            OuterPrivateVariables::new(N::inner_circuit_verifying_key().clone(), inner_proof, execution);

        let transaction_proof = N::OuterSNARK::prove(
            N::outer_circuit_proving_key(),
            &OuterCircuit::<N>::new(outer_public_variables, outer_private_variables),
            rng,
        )?;

        let metadata = TransactionMetadata::new(block_hash, *N::inner_circuit_id());
        end_timer!(execution_timer);

        Transaction::from(kernel, metadata, encrypted_records, transaction_proof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snarkvm_utilities::{FromBytes, ToBytes};

    use rand::SeedableRng;
    use rand_chacha::ChaChaRng;

    fn transaction_authorization_serialization_test<N: Network>() {
        let mut rng = ChaChaRng::seed_from_u64(1231275789u64);

        let recipient = Account::new(&mut rng).unwrap();
        let amount = AleoAmount::from_bytes(10);
        let state = StateTransition::new_coinbase(recipient.address, amount, &mut rng).unwrap();
        let authorization = DPC::<N>::authorize(&vec![], &state, &mut rng).unwrap();

        // Serialize and deserialize the transaction authorization.
        let deserialized_authorization = FromBytes::read_le(&authorization.to_bytes_le().unwrap()[..]).unwrap();
        assert_eq!(authorization, deserialized_authorization);
    }

    #[test]
    fn test_transaction_authorization_serialization() {
        transaction_authorization_serialization_test::<crate::testnet1::Testnet1>();
        transaction_authorization_serialization_test::<crate::testnet2::Testnet2>();
    }
}
