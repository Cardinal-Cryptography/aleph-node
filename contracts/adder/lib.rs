#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract]
mod adder {
    /// Defines the storage of your contract.
    /// Add new fields to the below struct in order
    /// to add new static storage fields to your contract.
    #[ink(storage)]
    pub struct Adder {
        /// Stores a single `bool` value on the storage.
        value: u32,
    }

    #[ink(event)]
    pub struct ValueChanged {
        new_value: u32,
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        Overflow,
    }

    impl Adder {
        /// Constructor that initializes the `bool` value to `false`.
        ///
        /// Constructors can delegate to other constructors.
        #[ink(constructor)]
        pub fn new() -> Self {
            Self { value: 0 }
        }

        /// A message that can be called on instantiated contracts.
        /// This one flips the value of the stored `bool` from `true`
        /// to `false` and vice versa.
        #[ink(message)]
        pub fn add(&mut self, value: u32) -> Result<(), Error> {
            self.value = self.value.checked_add(value).ok_or(Error::Overflow)?;

            Self::env().emit_event(ValueChanged {
                new_value: self.value,
            });

            Ok(())
        }

        /// Simply returns the current value of our `bool`.
        #[ink(message)]
        pub fn get(&self) -> u32 {
            self.value
        }
    }
}
