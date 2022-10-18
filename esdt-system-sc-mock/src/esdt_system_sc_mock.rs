#![no_std]

elrond_wasm::imports!();

const ZERO_ASCII: u8 = b'0';
const DASH: u8 = b'-';
const RAND_CHARS_LEN: usize = 6;

#[elrond_wasm::contract]
pub trait PayableFeatures {
    #[init]
    fn init(&self) {} // function executed when deploying the contract, this one return itself

    #[payable("EGLD")] // Needs to be paid 0.05 EGLD to create a new token
    #[endpoint(issue)] // data -> issue -> create a new token, completely flexible, it can be a NFT/SFT collection, with or without decimal quantity and with or without properties
    fn issue_fungible(
        &self,
        _token_display_name: ManagedBuffer,
        token_ticker: ManagedBuffer,
        initial_supply: BigUint,
        _num_decimals: usize,
        _token_properties: MultiValueEncoded<MultiValue2<ManagedBuffer, bool>>,
    ) -> TokenIdentifier {
        let new_token_id = self.create_new_token_id(token_ticker);
        require!(new_token_id.is_valid_esdt_identifier(), "Invalid token ID");

        if initial_supply > 0 {
            let caller = self.blockchain().get_caller();

            self.send()
                .esdt_local_mint(&new_token_id, 0, &initial_supply);
            self.send().transfer_esdt_via_async_call(
                caller,
                new_token_id.clone(),
                0,
                initial_supply,
            );
        }

        new_token_id
    }

    #[endpoint(setSpecialRole)] // endpoint to set new roles (permissions) concerning the token (like the right of creating new tokens into a collection)
    fn set_special_roles(
        // the function in charge of it
        &self,
        _token_id: TokenIdentifier,
        _address: ManagedAddress,
        _roles: MultiValueEncoded<EsdtLocalRole>,
    ) {
    }

    #[payable("EGLD")] // Needs to be paid 0.05EGLD
    #[endpoint(registerAndSetAllRoles)] // Not sure to know what this is doing but woow, there is no property into input, maybe an update function
    fn register_and_set_all_roles(
        &self,
        _token_display_name: ManagedBuffer,
        token_ticker: ManagedBuffer,
        _token_type_name: ManagedBuffer,
        _num_decimals: usize,
    ) -> TokenIdentifier {
        self.create_new_token_id(token_ticker)
    }

    fn create_new_token_id(&self, token_ticker: ManagedBuffer) -> TokenIdentifier {
        let nr_issued_tokens = self.nr_issued_tokens().get(); // get the map of issued tokens from this contract, then get a stored value of the last contract adress ???
        let mut rand_chars = [ZERO_ASCII; RAND_CHARS_LEN]; // creating an array with correct length for generating random chars, this array is directly fullfilled with 0 ascii chars
        for c in &mut rand_chars {
            *c += nr_issued_tokens; // randomization of the chars
        }

        self.nr_issued_tokens().update(|nr| *nr += 1); // Not sure, maybe we increment 

        let mut token_id = token_ticker; // Appending the final
        token_id.append_bytes(&[DASH][..]); // add '-'
        token_id.append_bytes(&rand_chars); // add the rand chars

        token_id.into() // into() is a method from into trait that convert a value from a type to another, here we jus return a "ManagedBuffer<<Self as ContractBase>::Api>" value type to a TokenIdentifier value type
    }

    #[storage_mapper("nrIssuedTokens")]
    fn nr_issued_tokens(&self) -> SingleValueMapper<u8>;
}
