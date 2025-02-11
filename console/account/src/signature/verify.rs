// Copyright (C) 2019-2022 Aleo Systems Inc.
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

use super::*;

impl<N: Network> Signature<N> {
    /// Verifies (challenge == challenge') && (address == address') where:
    ///     challenge' := HashToScalar(G^response pk_sig^challenge, pk_sig, pr_sig, address, message)
    pub fn verify(&self, address: &Address<N>, message: &[Field<N>]) -> bool {
        // Ensure the number of field elements does not exceed the maximum allowed size.
        if message.len() > N::MAX_DATA_SIZE_IN_FIELDS as usize {
            eprintln!("Cannot sign the signature: the signed message exceeds maximum allowed size");
            return false;
        }

        // Retrieve pk_sig.
        let pk_sig = self.compute_key.pk_sig();
        // Retrieve pr_sig.
        let pr_sig = self.compute_key.pr_sig();

        // Compute `g_r` := (response * G) + (challenge * pk_sig).
        let g_r = N::g_scalar_multiply(&self.response) + (pk_sig * self.challenge);

        // Construct the hash input as (r * G, pk_sig, pr_sig, address, message).
        let mut preimage = Vec::with_capacity(4 + message.len());
        preimage.extend([g_r, pk_sig, pr_sig, **address].map(|point| point.to_x_coordinate()));
        preimage.extend(message);

        // Hash to derive the verifier challenge, and return `false` if this operation fails.
        let candidate_challenge = match N::hash_to_scalar_psd8(&preimage) {
            // Output the computed candidate challenge.
            Ok(candidate_challenge) => candidate_challenge,
            // Return `false` if the challenge errored.
            Err(_) => return false,
        };

        // Derive the address from the compute key, and return `false` if this operation fails.
        let candidate_address = match Address::try_from(self.compute_key) {
            // Output the computed candidate address.
            Ok(candidate_address) => candidate_address,
            // Return `false` if the address errored.
            Err(_) => return false,
        };

        // Return `true` if the candidate challenge and address are correct.
        self.challenge == candidate_challenge && *address == candidate_address
    }

    /// Verifies a signature for the given address and message (as bytes).
    pub fn verify_bytes(&self, address: &Address<N>, message: &[u8]) -> bool {
        // Convert the message into bits, and verify the signature.
        self.verify_bits(address, &message.to_bits_le())
    }

    /// Verifies a signature for the given address and message (as bits).
    pub fn verify_bits(&self, address: &Address<N>, message: &[bool]) -> bool {
        // Pack the bits into field elements.
        match message.chunks(Field::<N>::size_in_data_bits()).map(Field::from_bits_le).collect::<Result<Vec<_>>>() {
            Ok(fields) => self.verify(address, &fields),
            Err(error) => {
                eprintln!("Failed to verify signature: {error}");
                false
            }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "private_key")]
mod tests {
    use super::*;
    use snarkvm_console_network::Testnet3;

    type CurrentNetwork = Testnet3;

    const ITERATIONS: u64 = 100;

    #[test]
    fn test_sign_and_verify() -> Result<()> {
        let rng = &mut test_crypto_rng();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Check that the signature is valid for the message.
            let message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            let signature = Signature::sign(&private_key, &message, rng)?;
            assert!(signature.verify(&address, &message));

            // Check that the signature is invalid for an incorrect message.
            let failure_message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            if message != failure_message {
                assert!(!signature.verify(&address, &failure_message));
            }
        }
        Ok(())
    }

    #[test]
    fn test_sign_and_verify_bytes() -> Result<()> {
        let rng = &mut test_crypto_rng();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Check that the signature is valid for the message.
            let message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            let signature = Signature::sign_bytes(&private_key, &message, rng)?;
            assert!(signature.verify_bytes(&address, &message));

            // Check that the signature is invalid for an incorrect message.
            let failure_message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            if message != failure_message {
                assert!(!signature.verify_bytes(&address, &failure_message));
            }
        }
        Ok(())
    }

    #[test]
    fn test_sign_and_verify_bits() -> Result<()> {
        let rng = &mut test_crypto_rng();

        for i in 0..ITERATIONS {
            // Sample an address and a private key.
            let private_key = PrivateKey::<CurrentNetwork>::new(rng)?;
            let address = Address::try_from(&private_key)?;

            // Check that the signature is valid for the message.
            let message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            let signature = Signature::sign_bits(&private_key, &message, rng)?;
            assert!(signature.verify_bits(&address, &message));

            // Check that the signature is invalid for an incorrect message.
            let failure_message: Vec<_> = (0..i).map(|_| Uniform::rand(rng)).collect();
            if message != failure_message {
                assert!(!signature.verify_bits(&address, &failure_message));
            }
        }
        Ok(())
    }
}
