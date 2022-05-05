elrond_wasm::imports!();

use crate::{
    brand_creation::INVALID_BRAND_ID_ERR_MSG,
    common_storage::{BrandId, BrandInfo, MintPrice, PaymentsVec},
    events::DestAddressAmountPair,
    nft_tier::TierName,
};

const NFT_AMOUNT: u32 = 1;

#[elrond_wasm::module]
pub trait NftMintingModule:
    crate::common_storage::CommonStorageModule
    + crate::nft_tier::NftTierModule
    + crate::royalties::RoyaltiesModule
    + crate::admin_whitelist::AdminWhitelistModule
    + crate::nft_attributes_builder::NftAttributesBuilderModule
    + crate::events::EventsModule
{
    #[payable("*")]
    #[endpoint(buyRandomNft)]
    fn buy_random_nft(
        &self,
        brand_id: BrandId<Self::Api>,
        tier: TierName<Self::Api>,
        opt_nfts_to_buy: OptionalValue<usize>,
    ) -> PaymentsVec<Self::Api> {
        require!(
            self.registered_brands().contains(&brand_id),
            INVALID_BRAND_ID_ERR_MSG
        );
        require!(
            self.nft_tiers_for_brand(&brand_id).contains(&tier),
            "Invalid tier"
        );

        let nfts_to_buy = match opt_nfts_to_buy {
            OptionalValue::Some(val) => {
                if val == 0 {
                    return PaymentsVec::new();
                }

                let max_nfts_per_transaction = self.max_nfts_per_transaction().get();
                require!(
                    val <= max_nfts_per_transaction,
                    "Max NFTs per transaction limit exceeded"
                );

                val
            }
            OptionalValue::None => NFT_AMOUNT as usize,
        };

        let price_for_tier: MintPrice<Self::Api> = self.price_for_tier(&brand_id, &tier).get();
        let payment: EsdtTokenPayment<Self::Api> = self.call_value().payment();
        let total_required_amount = &price_for_tier.amount * (nfts_to_buy as u32);
        require!(
            payment.token_identifier == price_for_tier.token_id
                && payment.amount == total_required_amount,
            "Invalid payment"
        );

        let brand_info: BrandInfo<Self::Api> = self.brand_info(&brand_id).get();
        let current_timestamp = self.blockchain().get_block_timestamp();
        require!(
            current_timestamp >= brand_info.mint_period.start,
            "May not mint yet"
        );
        require!(
            current_timestamp < brand_info.mint_period.end,
            "May not mint after deadline"
        );

        self.add_mint_payment(payment.token_identifier, payment.amount);

        let caller = self.blockchain().get_caller();
        let output_payments =
            self.mint_and_send_random_nft(&caller, &brand_id, &tier, &brand_info, nfts_to_buy);

        self.nft_bought_event(&caller, &brand_id, &tier, nfts_to_buy);

        output_payments
    }

    #[endpoint(giveawayNfts)]
    fn giveaway_nfts(
        &self,
        brand_id: BrandId<Self::Api>,
        tier: TierName<Self::Api>,
        dest_amount_pairs: MultiValueEncoded<MultiValue2<ManagedAddress, usize>>,
    ) {
        self.require_caller_is_admin();

        require!(
            self.registered_brands().contains(&brand_id),
            INVALID_BRAND_ID_ERR_MSG
        );
        require!(
            self.nft_tiers_for_brand(&brand_id).contains(&tier),
            "Invalid tier"
        );

        let mut arg_pairs = ManagedVec::new();
        let brand_info = self.brand_info(&brand_id).get();
        for pair in dest_amount_pairs {
            let (dest_address, nfts_to_send) = pair.into_tuple();
            if nfts_to_send > 0 {
                let _ = self.mint_and_send_random_nft(
                    &dest_address,
                    &brand_id,
                    &tier,
                    &brand_info,
                    nfts_to_send,
                );

                arg_pairs.push(DestAddressAmountPair {
                    dest_address,
                    nft_amount: nfts_to_send,
                });
            }
        }

        self.nft_giveaway_event(&brand_id, &tier, arg_pairs);
    }

    fn mint_and_send_random_nft(
        &self,
        to: &ManagedAddress,
        brand_id: &BrandId<Self::Api>,
        tier: &TierName<Self::Api>,
        brand_info: &BrandInfo<Self::Api>,
        nfts_to_send: usize,
    ) -> PaymentsVec<Self::Api> {
        let total_available_nfts = self.available_ids(brand_id, tier).len();
        require!(
            nfts_to_send <= total_available_nfts,
            "Not enough NFTs available"
        );

        let nft_token_id = self.nft_token(brand_id).get_token_id();
        let mut nft_output_payments = ManagedVec::new();
        for _ in 0..nfts_to_send {
            let nft_id = self.get_next_random_id(brand_id, tier);
            let nft_uri = self.build_nft_main_file_uri(
                &brand_info.collection_hash,
                nft_id,
                &brand_info.media_type,
            );
            let nft_json = self.build_nft_json_file_uri(&brand_info.collection_hash, nft_id);
            let collection_json = self.build_collection_json_file_uri(&brand_info.collection_hash);

            let mut uris = ManagedVec::new();
            uris.push(nft_uri);
            uris.push(nft_json);
            uris.push(collection_json);

            let attributes =
                self.build_nft_attributes(&brand_info.collection_hash, brand_id, nft_id);
            let nft_amount = BigUint::from(NFT_AMOUNT);
            let nft_nonce = self.send().esdt_nft_create(
                &nft_token_id,
                &nft_amount,
                &brand_info.token_display_name,
                &brand_info.royalties,
                &ManagedBuffer::new(),
                &attributes,
                &uris,
            );

            nft_output_payments.push(EsdtTokenPayment::new(
                nft_token_id.clone(),
                nft_nonce,
                nft_amount,
            ));
        }

        self.send().direct_multi(to, &nft_output_payments, &[]);

        nft_output_payments
    }
}
