// This file is part of Substrate.

// Copyright (C) 2017-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Assets Pallet
//!
//! A simple, secure module for dealing with fungible assets.
//!
//! ## Overview
//!
//! The Assets module provides functionality for asset management of fungible asset classes
//! with a fixed supply, including:
//!
//! * Asset Issuance (Minting)
//! * Asset Transferal
//! * Asset Freezing
//! * Asset Destruction (Burning)
//! * Delegated Asset Transfers ("Approval API")
//!
//! To use it in your runtime, you need to implement the assets [`Config`].
//!
//! The supported dispatchable functions are documented in the [`Call`] enum.
//!
//! ### Terminology
//!
//! * **Admin**: An account ID uniquely privileged to be able to unfreeze (thaw) an account and it's
//!   assets, as well as forcibly transfer a particular class of assets between arbitrary accounts
//!   and reduce the balance of a particular class of assets of arbitrary accounts.
//! * **Asset issuance/minting**: The creation of a new asset, whose total supply will belong to the
//!   account that issues the asset. This is a privileged operation.
//! * **Asset transfer**: The reduction of the balance of an asset of one account with the
//!   corresponding increase in the balance of another.
//! * **Asset destruction**: The process of reduce the balance of an asset of one account. This is a
//!   privileged operation.
//! * **Fungible asset**: An asset whose units are interchangeable.
//! * **Issuer**: An account ID uniquely privileged to be able to mint a particular class of assets.
//! * **Freezer**: An account ID uniquely privileged to be able to freeze an account from
//!   transferring a particular class of assets.
//! * **Freezing**: Removing the possibility of an unpermissioned transfer of an asset from a
//!   particular account.
//! * **Non-fungible asset**: An asset for which each unit has unique characteristics.
//! * **Owner**: An account ID uniquely privileged to be able to destroy a particular asset class,
//!   or to set the Issuer, Freezer or Admin of that asset class.
//! * **Approval**: The act of allowing an account the permission to transfer some balance of asset
//!   from the approving account into some third-party destination account.
//! * **Sufficiency**: The idea of a minimum-balance of an asset being sufficient to allow the
//!   account's existence on the system without requiring any other existential-deposit.
//!
//! ### Goals
//!
//! The assets system in Substrate is designed to make the following possible:
//!
//! * Issue a new assets in a permissioned or permissionless way, if permissionless, then with a
//!   deposit required.
//! * Allow accounts to be delegated the ability to transfer assets without otherwise existing
//!   on-chain (*approvals*).
//! * Move assets between accounts.
//! * Update the asset's total supply.
//! * Allow administrative activities by specially privileged accounts including freezing account
//!   balances and minting/burning assets.
//!
//! ## Interface
//!
//! ### Permissionless Functions
//!
//! * `create`: Creates a new asset class, taking the required deposit.
//! * `transfer`: Transfer sender's assets to another account.
//! * `transfer_keep_alive`: Transfer sender's assets to another account, keeping the sender alive.
//! * `approve_transfer`: Create or increase an delegated transfer.
//! * `cancel_approval`: Rescind a previous approval.
//! * `transfer_approved`: Transfer third-party's assets to another account.
//!
//! ### Permissioned Functions
//!
//! * `force_create`: Creates a new asset class without taking any deposit.
//! * `force_set_metadata`: Set the metadata of an asset class.
//! * `force_clear_metadata`: Remove the metadata of an asset class.
//! * `force_asset_status`: Alter an asset class's attributes.
//! * `force_cancel_approval`: Rescind a previous approval.
//!
//! ### Privileged Functions
//! * `destroy`: Destroys an entire asset class; called by the asset class's Owner.
//! * `mint`: Increases the asset balance of an account; called by the asset class's Issuer.
//! * `burn`: Decreases the asset balance of an account; called by the asset class's Admin.
//! * `force_transfer`: Transfers between arbitrary accounts; called by the asset class's Admin.
//! * `freeze`: Disallows further `transfer`s from an account; called by the asset class's Freezer.
//! * `thaw`: Allows further `transfer`s from an account; called by the asset class's Admin.
//! * `transfer_ownership`: Changes an asset class's Owner; called by the asset class's Owner.
//!   Owner.
//!
//! Please refer to the [`Call`] enum and its associated variants for documentation on each
//! function.
//!
//! ### Public Functions
//! <!-- Original author of descriptions: @gavofyork -->
//!
//! * `balance` - Get the asset `id` balance of `who`.
//! * `total_supply` - Get the total supply of an asset `id`.
//!
//! Please refer to the [`Pallet`] struct for details on publicly available functions.
//!
//! ## Related Modules
//!
//! * [`System`](../frame_system/index.html)
//! * [`Support`](../frame_support/index.html)

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

mod extra_mutator;
pub use extra_mutator::*;
mod functions;
mod impl_fungibles;
mod impl_stored_map;
mod types;
pub use types::*;

use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		AtLeast32BitUnsigned, Bounded, CheckedAdd, CheckedSub, Saturating, StaticLookup, Zero, One,
	},
	ArithmeticError, TokenError,
};
use sp_std::{borrow::Borrow, prelude::*};

use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	ensure,
	pallet_prelude::DispatchResultWithPostInfo,
	traits::{
		tokens::{fungibles, DepositConsequence, WithdrawConsequence},
		BalanceStatus::Reserved,
		Currency, ReservableCurrency, StoredMap,
	},
};
use frame_system::Config as SystemConfig;

pub use pallet::*;
pub use weights::WeightInfo;
pub type AssetId = u64;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::{*, StorageValue};
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::config]
	/// The module configuration trait.
	pub trait Config<I: 'static = ()>: frame_system::Config + pallet_timestamp::Config {
		/// The overarching event type.
		type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;

		/// The units in which we record balances.
		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// The currency mechanism.
		type Currency: ReservableCurrency<Self::AccountId>;

		/// The origin which may forcibly create or destroy an asset or otherwise alter privileged
		/// attributes.
		type ForceOrigin: EnsureOrigin<Self::Origin>;

		/// The basic amount of funds that must be reserved for an asset.
		#[pallet::constant]
		type AssetDeposit: Get<DepositBalanceOf<Self, I>>;

		/// The amount of funds that must be reserved for a non-provider asset account to be
		/// maintained.
		#[pallet::constant]
		type AssetAccountDeposit: Get<DepositBalanceOf<Self, I>>;

		/// The basic amount of funds that must be reserved when adding metadata to your asset.
		#[pallet::constant]
		type MetadataDepositBase: Get<DepositBalanceOf<Self, I>>;

		/// The additional funds that must be reserved for the number of bytes you store in your
		/// metadata.
		#[pallet::constant]
		type MetadataDepositPerByte: Get<DepositBalanceOf<Self, I>>;

		/// The amount of funds that must be reserved when creating a new approval.
		#[pallet::constant]
		type ApprovalDeposit: Get<DepositBalanceOf<Self, I>>;

		/// The maximum length of a name or symbol stored on-chain.
		#[pallet::constant]
		type StringLimit: Get<u32>;

		/// A hook to allow a per-asset, per-account minimum balance to be enforced. This must be
		/// respected in all permissionless operations.
		type Freezer: FrozenBalance<AssetId, Self::AccountId, Self::Balance>;

		/// Additional data to be stored with an account's asset balance.
		type Extra: Member + Parameter + Default + MaxEncodedLen;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		/// Randomness for asssets name generation
        type Randomness: frame_support::traits::Randomness<Self::Hash, Self::BlockNumber>;
	}

	#[pallet::storage]
	/// Details of an asset.
	pub(super) type Asset<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Blake2_128Concat,
		AssetId,
		AssetDetails<T::Balance, T::AccountId, DepositBalanceOf<T, I>>,
	>;

	#[pallet::storage]
	/// The holdings of a specific account for a specific asset.
	pub(super) type Account<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		AssetId,
		Blake2_128Concat,
		T::AccountId,
		AssetAccountOf<T, I>,
		OptionQuery,
		GetDefault,
		ConstU32<300_000>,
	>;

	#[pallet::storage]
	/// Approved balance transfers. First balance is the amount approved for transfer. Second
	/// is the amount of `T::Currency` reserved for storing this.
	/// First key is the asset ID, second key is the owner and third key is the delegate.
	pub(super) type Approvals<T: Config<I>, I: 'static = ()> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, AssetId>,
			NMapKey<Blake2_128Concat, T::AccountId>, // owner
			NMapKey<Blake2_128Concat, T::AccountId>, // delegate
		),
		Approval<T::Balance, DepositBalanceOf<T, I>>,
		OptionQuery,
		GetDefault,
		ConstU32<300_000>,
	>;

	#[pallet::storage]
	/// Metadata of an asset.
	pub(super) type Metadata<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Blake2_128Concat,
		AssetId,
		AssetMetadata<DepositBalanceOf<T, I>, BoundedVec<u8, T::StringLimit>>,
		ValueQuery,
		GetDefault,
		ConstU32<300_000>,
	>;

	#[pallet::storage]
	/// Burn certificates for an AccountId.
	pub(super) type BurnCertificate<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		AssetId,
		T::Balance
	>;

	#[pallet::storage]
	/// Evercity custodian - only custodian can mint or burn assets
	pub(super) type Custodian<T: Config<I>, I: 'static = ()> = StorageValue<
		_,
		T::AccountId
	>;

	#[pallet::storage]
	#[pallet::getter(fn get_last_id)]
	/// Last created AssetId
	pub(super) type LastAssetId<T: Config<I>, I: 'static = ()> = StorageValue<
		_,
		AssetId,
		ValueQuery,
		InitialAssetId
	>;

	#[pallet::type_value]
	pub(super) fn InitialAssetId() -> AssetId { 100u64 }

	/// Carbon asset Lots registry - for every AccountId and AssetId
	#[pallet::storage]
	#[pallet::getter(fn lots)]
	pub(super) type CarbonAssetLotRegistry<T: Config<I>, I: 'static = ()> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, AssetId>,
			NMapKey<Blake2_128Concat, T::AccountId>, // owner
			NMapKey<Blake2_128Concat, T::Moment>, // expiration date
		),
		CarbonAssetLot<T::AccountId, T::Balance, T::Balance>,
		OptionQuery
	>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
		/// Genesis custodian: custodian_address
		pub custodian: Option<T::AccountId>,
		/// Genesis assets: id, owner, is_sufficient, min_balance
		pub assets: Vec<(AssetId, T::AccountId, bool, T::Balance)>,
		/// Genesis metadata: id, name, symbol, decimals
		pub metadata: Vec<(AssetId, Vec<u8>, Vec<u8>, u8)>,
		/// Genesis accounts: id, account_id, balance
		pub accounts: Vec<(AssetId, T::AccountId, T::Balance)>,
	}

	#[cfg(feature = "std")]
	impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
		fn default() -> Self {
			Self {
				custodian: None,
				assets: Default::default(),
				metadata: Default::default(),
				accounts: Default::default(),
			}
		}
	}


	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig<T, I> {
		fn build(&self) {
			if let Some(custodian_account) = &self.custodian {
				Custodian::<T, I>::put(custodian_account);
			}

			for (id, owner, is_sufficient, min_balance) in &self.assets {
				assert!(!Asset::<T, I>::contains_key(id), "Asset id already in use");
				assert!(!min_balance.is_zero(), "Min balance should not be zero");
				Asset::<T, I>::insert(
					id,
					AssetDetails {
						owner: owner.clone(),
						issuer: owner.clone(),
						admin: owner.clone(),
						freezer: owner.clone(),
						supply: Zero::zero(),
						deposit: Zero::zero(),
						min_balance: *min_balance,
						is_sufficient: *is_sufficient,
						accounts: 0,
						sufficients: 0,
						approvals: 0,
						is_frozen: false,
					},
				);
			}

			for (id, name, symbol, decimals) in &self.metadata {
				assert!(Asset::<T, I>::contains_key(id), "Asset does not exist");

				let bounded_name: BoundedVec<u8, T::StringLimit> =
					name.clone().try_into().expect("asset name is too long");
				let bounded_symbol: BoundedVec<u8, T::StringLimit> =
					symbol.clone().try_into().expect("asset symbol is too long");
				let bounded_url: BoundedVec<u8, T::StringLimit> =
					"".as_bytes().to_vec().clone().try_into().expect("wrong url");
				let bounded_data_ipfs: BoundedVec<u8, T::StringLimit> =
					"".as_bytes().to_vec().clone().try_into().expect("wrong data_ipfs");
		
				let metadata = AssetMetadata {
					deposit: Zero::zero(),
					url: bounded_url,
					data_ipfs: bounded_data_ipfs,
					name: bounded_name,
					symbol: bounded_symbol,
					decimals: *decimals,
					is_frozen: false,
				};
				Metadata::<T, I>::insert(id, metadata);
			}

			for (id, account_id, amount) in &self.accounts {
				let result = <Pallet<T, I>>::increase_balance(
					*id,
					account_id,
					*amount,
					|details| -> DispatchResult {
						debug_assert!(
							T::Balance::max_value() - details.supply >= *amount,
							"checked in prep; qed"
						);
						details.supply = details.supply.saturating_add(*amount);
						Ok(())
					},
				);
				assert!(result.is_ok());
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// Some asset class was created.
		Created { asset_id: AssetId, creator: T::AccountId },
		/// Some assets were issued.
		Issued { asset_id: AssetId, owner: T::AccountId, total_supply: T::Balance },
		/// Some assets were transferred.
		Transferred {
			asset_id: AssetId,
			from: T::AccountId,
			to: T::AccountId,
			amount: T::Balance,
		},
		/// Some assets were destroyed.
		Burned { asset_id: AssetId, owner: T::AccountId, balance: T::Balance },
		/// The management team changed.
		TeamChanged {
			asset_id: AssetId,
			issuer: T::AccountId,
			admin: T::AccountId,
			freezer: T::AccountId,
		},
		/// The owner changed.
		OwnerChanged { asset_id: AssetId, owner: T::AccountId },
		/// Some account `who` was frozen.
		Frozen { asset_id: AssetId, who: T::AccountId },
		/// Some account `who` was thawed.
		Thawed { asset_id: AssetId, who: T::AccountId },
		/// Some asset `asset_id` was frozen.
		AssetFrozen { asset_id: AssetId },
		/// Some asset `asset_id` was thawed.
		AssetThawed { asset_id: AssetId },
		/// An asset class was destroyed.
		Destroyed { asset_id: AssetId },
		/// Some asset class was force-created.
		ForceCreated { asset_id: AssetId, owner: T::AccountId },
		/// New metadata has been set for an asset.
		MetadataSet {
			asset_id: AssetId,
			name: Vec<u8>,
			symbol: Vec<u8>,
			decimals: u8,
			is_frozen: bool,
		},
		/// Metadata has been cleared for an asset.
		MetadataCleared { asset_id: AssetId },
		/// (Additional) funds have been approved for transfer to a destination account.
		ApprovedTransfer {
			asset_id: AssetId,
			source: T::AccountId,
			delegate: T::AccountId,
			amount: T::Balance,
		},
		/// An approval for account `delegate` was cancelled by `owner`.
		ApprovalCancelled { asset_id: AssetId, owner: T::AccountId, delegate: T::AccountId },
		/// An `amount` was transferred in its entirety from `owner` to `destination` by
		/// the approved `delegate`.
		TransferredApproved {
			asset_id: AssetId,
			owner: T::AccountId,
			delegate: T::AccountId,
			destination: T::AccountId,
			amount: T::Balance,
		},
		/// An asset has had its attributes changed by the `Force` origin.
		AssetStatusChanged { asset_id: AssetId },
		/// New custodian has been set by the `Force` origin.
		CustodianSet { custodian: T::AccountId},
		/// Metadata has been updated with `url` and `data_ipfs`.
		MetadataUpdated { asset_id: AssetId, url: Vec<u8>, data_ipfs: Vec<u8>},
		/// Carbon assets has been burned by the `account`.
		CarbonCreditsBurned { account: T::AccountId, asset_id: AssetId, amount: T::Balance },
		/// Carbon asset `lot` has been created by  the `account`.
		CarbonAssetLotCreated { 
			asset_id: AssetId, 
			account: T::AccountId, 
			expiration_date: T::Moment,
			lot: CarbonAssetLot<T::AccountId, T::Balance, T::Balance>
		},
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Account balance must be greater than or equal to the transfer amount.
		BalanceLow,
		/// The account to alter does not exist.
		NoAccount,
		/// The signing account has no permission to do the operation.
		NoPermission,
		/// The given asset ID is unknown.
		Unknown,
		/// The origin account is frozen.
		Frozen,
		/// The asset ID is already taken.
		InUse,
		/// Invalid witness data given.
		BadWitness,
		/// Minimum balance should be non-zero.
		MinBalanceZero,
		/// Unable to increment the consumer reference counters on the account. Either no provider
		/// reference exists to allow a non-zero balance of a non-self-sufficient asset, or the
		/// maximum number of consumers has been reached.
		NoProvider,
		/// Invalid metadata given.
		BadMetadata,
		/// No approval exists that would allow the transfer.
		Unapproved,
		/// The source account would not survive the transfer and it needs to stay alive.
		WouldDie,
		/// The asset-account already exists.
		AlreadyExists,
		/// The asset-account doesn't have an associated deposit.
		NoDeposit,
		/// The operation would result in funds being burned.
		WouldBurn,
		/// Operation can not be done, custodian need to be set.
		NoCustodian,
		/// Metadata for the asset does not exist.
		NoMetadata,
		/// Project data cannot be changed after minting.
		CannotChangeAfterMint,
		/// Deadline should be in the future.
		DeadlineIncorrect,
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		
		/// Sets new custodian.
		/// 
		/// The origin must conform to `ForceOrigin`.
		/// 
		/// - `custodian`: New custodian to be set. Only custodian can verify creation of carbon 
		/// credit asset and mint created carbon credit asset.
		/// 
		/// Emits `CustodianSet` when successful.
		/// 
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1, 1))]
		pub fn set_custodian(
			origin: OriginFor<T>,
			custodian: T::AccountId
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			Custodian::<T, I>::put(custodian.clone());
			Self::deposit_event(Event::CustodianSet { custodian });
			Ok(())
		}

		/// Issue a new class of fungible carbon assets from a public origin.
		///
		/// This new asset class has no assets initially and its owner is the origin.
		///
		/// The origin must be Signed and the sender must have sufficient funds free.
		///
		/// Funds of sender are reserved by `AssetDeposit`.
		/// 
		/// Admin of asset is the Custodian. Fails if no custodian are set.
		/// Set asset metadata: generated `name` and `symbol`, decimals to 9.
		/// 
		/// Emits `Created` event when successful.
		/// Emits `MetadataSet` with generated `name` and `symbol`.
		///
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let admin_option = Custodian::<T, I>::get();
			ensure!(admin_option.is_some(), Error::<T, I>::NoCustodian);
			let admin = admin_option.unwrap();
			let id = Self::get_new_asset_id()?;

			let deposit = T::AssetDeposit::get();
			T::Currency::reserve(&owner, deposit)?;

			Asset::<T, I>::insert(
				id,
				AssetDetails {
					owner: owner.clone(),
					issuer: admin.clone(),
					admin: admin.clone(),
					freezer: admin.clone(),
					supply: Zero::zero(),
					deposit,
					min_balance: One::one(),
					is_sufficient: false,
					accounts: 0,
					sufficients: 0,
					approvals: 0,
					is_frozen: false,
				},
			);
			Self::deposit_event(Event::Created { asset_id: id, creator: owner.clone() });
			let name = Self::construct_carbon_asset_name(&owner);
			let symbol = Self::construct_carbon_asset_symbol(&owner);
			Self::do_set_metadata(id, &owner, name, symbol, 9)
		}

		/// Set project data to metadata of an asset.
		/// 
		/// Origin must be Signed and the sender should be the Owner of the asset `id` or the Custodian.
		/// 
		/// - `id`: The identifier of the asset to update.
		/// - `url`: The url.
		/// - `data_ipfs`: The ipfs data link.
		/// 
		/// Emits `MetadataUpdated`.
		/// 
		#[pallet::weight(10_000_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn set_project_data(
			origin: OriginFor<T>,
			id: AssetId,
			url: Vec<u8>,
			data_ipfs: Vec<u8>,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;
			Self::update_metadata(id, &caller, url, data_ipfs)
		}

		/// Issue a new class of fungible assets from a privileged origin.
		///
		/// This new asset class has no assets initially.
		///
		/// The origin must conform to `ForceOrigin`.
		///
		/// Unlike `create`, no funds are reserved.
		///
		/// - `id`: The identifier of the new asset. This must not be currently in use to identify
		/// an existing asset.
		/// - `owner`: The owner of this class of assets. The owner has full superuser permissions
		/// over this asset, but may later change and configure the permissions using
		/// `transfer_ownership` and `set_team`.
		/// - `min_balance`: The minimum balance of this new asset that any single account must
		/// have. If an account's balance is reduced below this, then it collapses to zero.
		///
		/// Emits `ForceCreated` event when successful.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::force_create())]
		pub fn force_create(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			owner: <T::Lookup as StaticLookup>::Source,
			is_sufficient: bool,
			#[pallet::compact] min_balance: T::Balance,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;
			let owner = T::Lookup::lookup(owner)?;
			Self::do_force_create(id, owner, is_sufficient, min_balance)
		}

		/// Destroy a class of fungible assets.
		///
		/// The origin must conform to `ForceOrigin` or must be Signed and the sender must be the
		/// owner of the asset `id`.
		///
		/// - `id`: The identifier of the asset to be destroyed. This must identify an existing
		/// asset.
		///
		/// Emits `Destroyed` event when successful.
		///
		/// NOTE: It can be helpful to first freeze an asset before destroying it so that you
		/// can provide accurate witness information and prevent users from manipulating state
		/// in a way that can make it harder to destroy.
		///
		/// Weight: `O(c + p + a)` where:
		/// - `c = (witness.accounts - witness.sufficients)`
		/// - `s = witness.sufficients`
		/// - `a = witness.approvals`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::destroy(
			witness.accounts.saturating_sub(witness.sufficients),
 			witness.sufficients,
 			witness.approvals,
 		))]
		pub fn destroy(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			witness: DestroyWitness,
		) -> DispatchResultWithPostInfo {
			let maybe_check_owner = match T::ForceOrigin::try_origin(origin) {
				Ok(_) => None,
				Err(origin) => Some(ensure_signed(origin)?),
			};
			let details = Self::do_destroy(id, witness, maybe_check_owner)?;
			Ok(Some(<T as pallet::Config<I>>::WeightInfo::destroy(
				details.accounts.saturating_sub(details.sufficients),
				details.sufficients,
				details.approvals,
			))
			.into())
		}

		/// Mint carbon assets of a particular class by Custodian. Benefitiary is the owner of the asset.
		///
		/// The origin must be Signed and the sender must be the Custodian == the Issuer of the asset `id`.
		///
		/// - `id`: The identifier of the asset to have some amount minted.
		/// - `amount`: The amount of the asset to be minted.
		///
		/// Emits `Issued` event when successful.
		///
		/// Weight: `O(1)`
		/// 
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::mint())]
		pub fn mint(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			let asset_details = Asset::<T, I>::get(id).ok_or(Error::<T, I>::Unknown)?;
			let beneficiary = asset_details.owner;
			Self::do_mint(id, &beneficiary, amount, Some(origin))?;
			Ok(())
		}

		/// Burn of carbon credits assets by custodian. 
		/// Reduce the balance of `who` by as much as possible up to `amount` assets of `id`.
		/// Store information about the burned carbon asset in `BurnCertificate`.
		///
		/// Origin must be Signed and the sender should be the Custodian.
		///
		/// Bails with `NoAccount` if the `who` is already dead.
		///
		/// - `id`: The identifier of the asset to have some amount burned.
		/// - `who`: The account to be debited from.
		/// - `amount`: The maximum amount by which `who`'s balance should be reduced.
		///
		/// Emits `Burned` with the actual amount burned. If this takes the balance to below the
		/// minimum for the asset, then the amount burned is increased to take it to zero.
		/// 
		/// Emits `CarbonCreditsBurned`.
		///
		/// Weight: `O(1)`
		/// Modes: Post-existence of `who`; Pre & post Zombie-status of `who`.
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::burn())]
		pub fn burn(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			who: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			let who = T::Lookup::lookup(who)?;

			let f = DebitFlags { keep_alive: false, best_effort: false };
			let _ = Self::do_burn(id, &who, amount, Some(origin), f)?;

			BurnCertificate::<T,I>::mutate(who.clone(), id, |burned| {
				if let Some(b) = burned {
					let result = b.saturating_add(amount);
					*burned = Some(result);
				} else {
					*burned = Some(amount);
				}
			});
			Self::deposit_event(Event::CarbonCreditsBurned {account: who, asset_id: id, amount: amount});
			Ok(())
		}

		/// Burn of carbon credits assets by owner. 
		/// Reduce the balance of `who` by as much as possible up to `amount` assets of `id`.
		/// Store information about the burned carbon asset in `BurnCertificate`.
		///
		/// Origin must be Signed and the sender should have enough amount of asset.
		///
		/// Bails with `NoAccount` if the `who` is already dead.
		///
		/// - `id`: The identifier of the asset to have some amount burned.
		/// - `amount`: The maximum amount by which `who`'s balance should be reduced.
		///
		/// Emits `Burned` with the actual amount burned. If this takes the balance to below the
		/// minimum for the asset, then the amount burned is increased to take it to zero.
		/// 
		/// Emits `CarbonCreditsBurned`.
		///
		/// Weight: `O(1)`
		/// Modes: Post-existence of `who`; Pre & post Zombie-status of `who`.
		#[pallet::weight(10_000_000 + T::DbWeight::get().reads_writes(3,3))]
		pub fn self_burn(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;

			let f = DebitFlags { keep_alive: false, best_effort: false };
			let actual = Self::decrease_balance(id, &caller, amount, f, |actual, details| {
				details.supply = details.supply.saturating_sub(actual);

				Ok(())
			})?;
			Self::deposit_event(Event::Burned { asset_id: id, owner: caller.clone(), balance: actual });
		
			BurnCertificate::<T,I>::mutate(caller.clone(), id, |burned| {
				if let Some(b) = burned {
					let result = b.saturating_add(amount);
					*burned = Some(result);
				} else {
					*burned = Some(amount);
				}
			});
			Self::deposit_event(Event::CarbonCreditsBurned {account: caller, asset_id: id, amount: amount});
			Ok(())
		}

		/// Move some assets from the sender account to another.
		///
		/// Origin must be Signed.
		///
		/// - `id`: The identifier of the asset to have some amount transferred.
		/// - `target`: The account to be credited.
		/// - `amount`: The amount by which the sender's balance of assets should be reduced and
		/// `target`'s balance increased. The amount actually transferred may be slightly greater in
		/// the case that the transfer would otherwise take the sender balance above zero but below
		/// the minimum balance. Must be greater than zero.
		///
		/// Emits `Transferred` with the actual amount transferred. If this takes the source balance
		/// to below the minimum for the asset, then the amount transferred is increased to take it
		/// to zero.
		///
		/// Weight: `O(1)`
		/// Modes: Pre-existence of `target`; Post-existence of sender; Account pre-existence of
		/// `target`.
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			target: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			let dest = T::Lookup::lookup(target)?;

			let f = TransferFlags { keep_alive: false, best_effort: false, burn_dust: false };
			Self::do_transfer(id, &origin, &dest, amount, None, f).map(|_| ())
		}

		/// Move some assets from the sender account to another, keeping the sender account alive.
		///
		/// Origin must be Signed.
		///
		/// - `id`: The identifier of the asset to have some amount transferred.
		/// - `target`: The account to be credited.
		/// - `amount`: The amount by which the sender's balance of assets should be reduced and
		/// `target`'s balance increased. The amount actually transferred may be slightly greater in
		/// the case that the transfer would otherwise take the sender balance above zero but below
		/// the minimum balance. Must be greater than zero.
		///
		/// Emits `Transferred` with the actual amount transferred. If this takes the source balance
		/// to below the minimum for the asset, then the amount transferred is increased to take it
		/// to zero.
		///
		/// Weight: `O(1)`
		/// Modes: Pre-existence of `target`; Post-existence of sender; Account pre-existence of
		/// `target`.
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::transfer_keep_alive())]
		pub fn transfer_keep_alive(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			target: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			let source = ensure_signed(origin)?;
			let dest = T::Lookup::lookup(target)?;

			let f = TransferFlags { keep_alive: true, best_effort: false, burn_dust: false };
			Self::do_transfer(id, &source, &dest, amount, None, f).map(|_| ())
		}

		/// Move some assets from one account to another.
		///
		/// Origin must be Signed and the sender should be the Admin of the asset `id`.
		///
		/// - `id`: The identifier of the asset to have some amount transferred.
		/// - `source`: The account to be debited.
		/// - `dest`: The account to be credited.
		/// - `amount`: The amount by which the `source`'s balance of assets should be reduced and
		/// `dest`'s balance increased. The amount actually transferred may be slightly greater in
		/// the case that the transfer would otherwise take the `source` balance above zero but
		/// below the minimum balance. Must be greater than zero.
		///
		/// Emits `Transferred` with the actual amount transferred. If this takes the source balance
		/// to below the minimum for the asset, then the amount transferred is increased to take it
		/// to zero.
		///
		/// Weight: `O(1)`
		/// Modes: Pre-existence of `dest`; Post-existence of `source`; Account pre-existence of
		/// `dest`.
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::force_transfer())]
		pub fn force_transfer(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			source: <T::Lookup as StaticLookup>::Source,
			dest: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			let source = T::Lookup::lookup(source)?;
			let dest = T::Lookup::lookup(dest)?;

			let f = TransferFlags { keep_alive: false, best_effort: false, burn_dust: false };
			Self::do_transfer(id, &source, &dest, amount, Some(origin), f).map(|_| ())
		}

		/// Disallow further unprivileged transfers from an account.
		///
		/// Origin must be Signed and the sender should be the Freezer of the asset `id`.
		///
		/// - `id`: The identifier of the asset to be frozen.
		/// - `who`: The account to be frozen.
		///
		/// Emits `Frozen`.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::freeze())]
		pub fn freeze(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			who: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			let d = Asset::<T, I>::get(id).ok_or(Error::<T, I>::Unknown)?;
			ensure!(origin == d.freezer, Error::<T, I>::NoPermission);
			let who = T::Lookup::lookup(who)?;

			Account::<T, I>::try_mutate(id, &who, |maybe_account| -> DispatchResult {
				maybe_account.as_mut().ok_or(Error::<T, I>::NoAccount)?.is_frozen = true;
				Ok(())
			})?;

			Self::deposit_event(Event::<T, I>::Frozen { asset_id: id, who });
			Ok(())
		}

		/// Allow unprivileged transfers from an account again.
		///
		/// Origin must be Signed and the sender should be the Admin of the asset `id`.
		///
		/// - `id`: The identifier of the asset to be frozen.
		/// - `who`: The account to be unfrozen.
		///
		/// Emits `Thawed`.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::thaw())]
		pub fn thaw(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			who: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			let details = Asset::<T, I>::get(id).ok_or(Error::<T, I>::Unknown)?;
			ensure!(origin == details.admin, Error::<T, I>::NoPermission);
			let who = T::Lookup::lookup(who)?;

			Account::<T, I>::try_mutate(id, &who, |maybe_account| -> DispatchResult {
				maybe_account.as_mut().ok_or(Error::<T, I>::NoAccount)?.is_frozen = false;
				Ok(())
			})?;

			Self::deposit_event(Event::<T, I>::Thawed { asset_id: id, who });
			Ok(())
		}

		/// Disallow further unprivileged transfers for the asset class.
		///
		/// Origin must be Signed and the sender should be the Freezer of the asset `id`.
		///
		/// - `id`: The identifier of the asset to be frozen.
		///
		/// Emits `Frozen`.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::freeze_asset())]
		pub fn freeze_asset(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			Asset::<T, I>::try_mutate(id, |maybe_details| {
				let d = maybe_details.as_mut().ok_or(Error::<T, I>::Unknown)?;
				ensure!(origin == d.freezer, Error::<T, I>::NoPermission);

				d.is_frozen = true;

				Self::deposit_event(Event::<T, I>::AssetFrozen { asset_id: id });
				Ok(())
			})
		}

		/// Allow unprivileged transfers for the asset again.
		///
		/// Origin must be Signed and the sender should be the Admin of the asset `id`.
		///
		/// - `id`: The identifier of the asset to be thawed.
		///
		/// Emits `Thawed`.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::thaw_asset())]
		pub fn thaw_asset(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;

			Asset::<T, I>::try_mutate(id, |maybe_details| {
				let d = maybe_details.as_mut().ok_or(Error::<T, I>::Unknown)?;
				ensure!(origin == d.admin, Error::<T, I>::NoPermission);

				d.is_frozen = false;

				Self::deposit_event(Event::<T, I>::AssetThawed { asset_id: id });
				Ok(())
			})
		}

		/// Change the Owner of an asset.
		///
		/// Origin must be Signed and the sender should be the Owner of the asset `id`.
		///
		/// - `id`: The identifier of the asset.
		/// - `owner`: The new Owner of this asset.
		///
		/// Emits `OwnerChanged`.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::transfer_ownership())]
		pub fn transfer_ownership(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			owner: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let origin = ensure_signed(origin)?;
			let owner = T::Lookup::lookup(owner)?;

			Asset::<T, I>::try_mutate(id, |maybe_details| {
				let details = maybe_details.as_mut().ok_or(Error::<T, I>::Unknown)?;
				ensure!(origin == details.owner, Error::<T, I>::NoPermission);
				if details.owner == owner {
					return Ok(())
				}

				let metadata_deposit = Metadata::<T, I>::get(id).deposit;
				let deposit = details.deposit + metadata_deposit;

				// Move the deposit to the new owner.
				T::Currency::repatriate_reserved(&details.owner, &owner, deposit, Reserved)?;

				details.owner = owner.clone();

				Self::deposit_event(Event::OwnerChanged { asset_id: id, owner });
				Ok(())
			})
		}

		/// Force the metadata for an asset to some value.
		///
		/// Origin must be ForceOrigin.
		///
		/// Any deposit is left alone.
		///
		/// - `id`: The identifier of the asset to update.
		/// - `name`: The user friendly name of this asset. Limited in length by `StringLimit`.
		/// - `symbol`: The exchange symbol for this asset. Limited in length by `StringLimit`.
		/// - `decimals`: The number of decimals this asset uses to represent one unit.
		///
		/// Emits `MetadataSet`.
		///
		/// Weight: `O(N + S)` where N and S are the length of the name and symbol respectively.
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::force_set_metadata(name.len() as u32, symbol.len() as u32))]
		pub fn force_set_metadata(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			name: Vec<u8>,
			symbol: Vec<u8>,
			url: Vec<u8>,
			data_ipfs: Vec<u8>,
			decimals: u8,
			is_frozen: bool,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;

			let bounded_name: BoundedVec<u8, T::StringLimit> =
				name.clone().try_into().map_err(|_| Error::<T, I>::BadMetadata)?;

			let bounded_symbol: BoundedVec<u8, T::StringLimit> =
				symbol.clone().try_into().map_err(|_| Error::<T, I>::BadMetadata)?;

			let bounded_url: BoundedVec<u8, T::StringLimit> =
				url.clone().try_into().map_err(|_| Error::<T, I>::BadMetadata)?;
			let bounded_data_ipfs: BoundedVec<u8, T::StringLimit> =
				data_ipfs.clone().try_into().map_err(|_| Error::<T, I>::BadMetadata)?;

			ensure!(Asset::<T, I>::contains_key(id), Error::<T, I>::Unknown);
			Metadata::<T, I>::try_mutate_exists(id, |metadata| {
				let deposit = metadata.take().map_or(Zero::zero(), |m| m.deposit);
				*metadata = Some(AssetMetadata {
					deposit,
					url: bounded_url,
					data_ipfs: bounded_data_ipfs,
					name: bounded_name,
					symbol: bounded_symbol,
					decimals,
					is_frozen,
				});

				Self::deposit_event(Event::MetadataSet {
					asset_id: id,
					name,
					symbol,
					decimals,
					is_frozen,
				});
				Self::deposit_event(Event::MetadataUpdated {
					asset_id: id,
					url,
					data_ipfs,
				});
				Ok(())
			})
		}

		/// Clear the metadata for an asset.
		///
		/// Origin must be ForceOrigin.
		///
		/// Any deposit is returned.
		///
		/// - `id`: The identifier of the asset to clear.
		///
		/// Emits `MetadataCleared`.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::force_clear_metadata())]
		pub fn force_clear_metadata(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;

			let d = Asset::<T, I>::get(id).ok_or(Error::<T, I>::Unknown)?;
			Metadata::<T, I>::try_mutate_exists(id, |metadata| {
				let deposit = metadata.take().ok_or(Error::<T, I>::Unknown)?.deposit;
				T::Currency::unreserve(&d.owner, deposit);
				Self::deposit_event(Event::MetadataCleared { asset_id: id });
				Ok(())
			})
		}

		/// Alter the attributes of a given asset.
		///
		/// Origin must be `ForceOrigin`.
		///
		/// - `id`: The identifier of the asset.
		/// - `owner`: The new Owner of this asset.
		/// - `issuer`: The new Issuer of this asset.
		/// - `admin`: The new Admin of this asset.
		/// - `freezer`: The new Freezer of this asset.
		/// - `min_balance`: The minimum balance of this new asset that any single account must
		/// have. If an account's balance is reduced below this, then it collapses to zero.
		/// - `is_sufficient`: Whether a non-zero balance of this asset is deposit of sufficient
		/// value to account for the state bloat associated with its balance storage. If set to
		/// `true`, then non-zero balances may be stored without a `consumer` reference (and thus
		/// an ED in the Balances pallet or whatever else is used to control user-account state
		/// growth).
		/// - `is_frozen`: Whether this asset class is frozen except for permissioned/admin
		/// instructions.
		///
		/// Emits `AssetStatusChanged` with the identity of the asset.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::force_asset_status())]
		pub fn force_asset_status(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			owner: <T::Lookup as StaticLookup>::Source,
			issuer: <T::Lookup as StaticLookup>::Source,
			admin: <T::Lookup as StaticLookup>::Source,
			freezer: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] min_balance: T::Balance,
			is_sufficient: bool,
			is_frozen: bool,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;

			Asset::<T, I>::try_mutate(id, |maybe_asset| {
				let mut asset = maybe_asset.take().ok_or(Error::<T, I>::Unknown)?;
				asset.owner = T::Lookup::lookup(owner)?;
				asset.issuer = T::Lookup::lookup(issuer)?;
				asset.admin = T::Lookup::lookup(admin)?;
				asset.freezer = T::Lookup::lookup(freezer)?;
				asset.min_balance = min_balance;
				asset.is_sufficient = is_sufficient;
				asset.is_frozen = is_frozen;
				*maybe_asset = Some(asset);

				Self::deposit_event(Event::AssetStatusChanged { asset_id: id });
				Ok(())
			})
		}

		/// Approve an amount of asset for transfer by a delegated third-party account.
		///
		/// Origin must be Signed.
		///
		/// Ensures that `ApprovalDeposit` worth of `Currency` is reserved from signing account
		/// for the purpose of holding the approval. If some non-zero amount of assets is already
		/// approved from signing account to `delegate`, then it is topped up or unreserved to
		/// meet the right value.
		///
		/// NOTE: The signing account does not need to own `amount` of assets at the point of
		/// making this call.
		///
		/// - `id`: The identifier of the asset.
		/// - `delegate`: The account to delegate permission to transfer asset.
		/// - `amount`: The amount of asset that may be transferred by `delegate`. If there is
		/// already an approval in place, then this acts additively.
		///
		/// Emits `ApprovedTransfer` on success.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::approve_transfer())]
		pub fn approve_transfer(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			delegate: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let delegate = T::Lookup::lookup(delegate)?;
			Self::do_approve_transfer(id, &owner, &delegate, amount)
		}

		/// Cancel all of some asset approved for delegated transfer by a third-party account.
		///
		/// Origin must be Signed and there must be an approval in place between signer and
		/// `delegate`.
		///
		/// Unreserves any deposit previously reserved by `approve_transfer` for the approval.
		///
		/// - `id`: The identifier of the asset.
		/// - `delegate`: The account delegated permission to transfer asset.
		///
		/// Emits `ApprovalCancelled` on success.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::cancel_approval())]
		pub fn cancel_approval(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			delegate: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let delegate = T::Lookup::lookup(delegate)?;
			let mut d = Asset::<T, I>::get(id).ok_or(Error::<T, I>::Unknown)?;
			let approval =
				Approvals::<T, I>::take((id, &owner, &delegate)).ok_or(Error::<T, I>::Unknown)?;
			T::Currency::unreserve(&owner, approval.deposit);

			d.approvals.saturating_dec();
			Asset::<T, I>::insert(id, d);

			Self::deposit_event(Event::ApprovalCancelled { asset_id: id, owner, delegate });
			Ok(())
		}

		/// Cancel all of some asset approved for delegated transfer by a third-party account.
		///
		/// Origin must be either ForceOrigin or Signed origin with the signer being the Admin
		/// account of the asset `id`.
		///
		/// Unreserves any deposit previously reserved by `approve_transfer` for the approval.
		///
		/// - `id`: The identifier of the asset.
		/// - `delegate`: The account delegated permission to transfer asset.
		///
		/// Emits `ApprovalCancelled` on success.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::force_cancel_approval())]
		pub fn force_cancel_approval(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			owner: <T::Lookup as StaticLookup>::Source,
			delegate: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let mut d = Asset::<T, I>::get(id).ok_or(Error::<T, I>::Unknown)?;
			T::ForceOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(|origin| -> DispatchResult {
					let origin = ensure_signed(origin)?;
					ensure!(origin == d.admin, Error::<T, I>::NoPermission);
					Ok(())
				})?;

			let owner = T::Lookup::lookup(owner)?;
			let delegate = T::Lookup::lookup(delegate)?;

			let approval =
				Approvals::<T, I>::take((id, &owner, &delegate)).ok_or(Error::<T, I>::Unknown)?;
			T::Currency::unreserve(&owner, approval.deposit);
			d.approvals.saturating_dec();
			Asset::<T, I>::insert(id, d);

			Self::deposit_event(Event::ApprovalCancelled { asset_id: id, owner, delegate });
			Ok(())
		}

		/// Transfer some asset balance from a previously delegated account to some third-party
		/// account.
		///
		/// Origin must be Signed and there must be an approval in place by the `owner` to the
		/// signer.
		///
		/// If the entire amount approved for transfer is transferred, then any deposit previously
		/// reserved by `approve_transfer` is unreserved.
		///
		/// - `id`: The identifier of the asset.
		/// - `owner`: The account which previously approved for a transfer of at least `amount` and
		/// from which the asset balance will be withdrawn.
		/// - `destination`: The account to which the asset balance of `amount` will be transferred.
		/// - `amount`: The amount of assets to transfer.
		///
		/// Emits `TransferredApproved` on success.
		///
		/// Weight: `O(1)`
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::transfer_approved())]
		pub fn transfer_approved(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			owner: <T::Lookup as StaticLookup>::Source,
			destination: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			let delegate = ensure_signed(origin)?;
			let owner = T::Lookup::lookup(owner)?;
			let destination = T::Lookup::lookup(destination)?;
			Self::do_transfer_approved(id, &owner, &delegate, &destination, amount)
		}

		/// Create an asset account for non-provider assets.
		///
		/// A deposit will be taken from the signer account.
		///
		/// - `origin`: Must be Signed; the signer account must have sufficient funds for a deposit
		///   to be taken.
		/// - `id`: The identifier of the asset for the account to be created.
		///
		/// Emits `Touched` event when successful.
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::mint())]
		pub fn touch(origin: OriginFor<T>, #[pallet::compact] id: AssetId) -> DispatchResult {
			Self::do_touch(id, ensure_signed(origin)?)
		}

		/// Return the deposit (if any) of an asset account.
		///
		/// The origin must be Signed.
		///
		/// - `id`: The identifier of the asset for the account to be created.
		/// - `allow_burn`: If `true` then assets may be destroyed in order to complete the refund.
		///
		/// Emits `Refunded` event when successful.
		#[pallet::weight(<T as pallet::Config<I>>::WeightInfo::mint())]
		pub fn refund(
			origin: OriginFor<T>,
			#[pallet::compact] id: AssetId,
			allow_burn: bool,
		) -> DispatchResult {
			Self::do_refund(id, ensure_signed(origin)?, allow_burn)
		}

		/// Creates new Carbon asset Lot of given asset_id. 
		/// 
		///	- `asset_id`: CarbonCreditsId<T> - Carbon assets asset id
		/// - `deadline` - lot can be sold only before deadline
		///	- `new_lot`: CarbonAssetLot<T> - new lot of Carbon asset to auction off
		/// 
		/// Access: for Carbon asset holder
		/// 
		/// CarbonAssetLot new_lot contains:
		/// - `target_bearer` - optional, if set - lot is private
		/// - `amount` - amount of Carbon asset for sell
		/// - `total_price` - price of whole lot
		/// 
		/// Checks if `deadline` is correct, if caller has enough Carbon assets.
		/// Purges another expired lots for this caller.
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(3, 2))]
		pub fn create_carbon_asset_lot(
			origin: OriginFor<T>,
			#[pallet::compact] asset_id: AssetId,
			deadline: T::Moment,
			new_lot: CarbonAssetLot<T::AccountId, T::Balance, T::Balance>,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;
			let now = pallet_timestamp::Pallet::<T>::get();
			ensure!(deadline >= now, Error::<T, I>::DeadlineIncorrect);

			Account::<T,I>::try_mutate(asset_id, &caller, |data_opt| -> DispatchResult {
				match data_opt {
					None => Err(Error::<T,I>::BalanceLow)?,
					Some(data) => {
						// remove expired lot and unreserve balance
						let unreserved_opt = Self::remove_expired_lots(asset_id, &caller, now);
						if let Some(unreserved) = unreserved_opt {
							data.balance = data.balance.checked_add(&unreserved).ok_or(ArithmeticError::Overflow)?;
						}
						
						ensure!(data.balance >= new_lot.amount, Error::<T,I>::BalanceLow);

						CarbonAssetLotRegistry::<T,I>::insert((asset_id, &caller, deadline), new_lot.clone());

						// reserve assets for lot
						data.reserved_balance = data.reserved_balance.checked_add(&new_lot.amount).ok_or(ArithmeticError::Overflow)?;
						data.balance = data.balance.saturating_sub(new_lot.amount);
						Ok(())
					}
				}
			})?;
			Ok(())
		}


		/// Expires and deletes a specified lot. All others expired lots are also deleted.
		/// 
		/// - `asset_id`: AssetId - Lot asset id 
		/// - `deadline`: T::Moment - Lot deadline
		/// 
		/// Access: lot owner
		///  
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(3, 2))]
		pub fn expire_lot(
			origin: OriginFor<T>,
			#[pallet::compact] asset_id: AssetId,
			deadline: T::Moment,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;
			let now = pallet_timestamp::Pallet::<T>::get();

			Account::<T,I>::try_mutate(asset_id, &caller, |data_opt| -> DispatchResult {
				match data_opt {
					None => Err(Error::<T,I>::BalanceLow)?,
					Some(data) => {
						// remove expired lot and unreserve balance
						let unreserved_opt = Self::remove_expired_lots(asset_id, &caller, now);
						if let Some(unreserved) = unreserved_opt {
							data.balance = data.balance.checked_add(&unreserved).ok_or(ArithmeticError::Overflow)?;
						}
						
						// remove lot and unreserve assets for lot
						let lot_opt = CarbonAssetLotRegistry::<T,I>::take((asset_id, &caller, deadline));
						if let Some(lot) = lot_opt {
							data.reserved_balance = data.reserved_balance.saturating_sub(lot.amount);
							data.balance = data.balance.checked_add(&lot.amount).ok_or(ArithmeticError::Overflow)?;
						}		
						Ok(())
					}
				}
			})?;
			Ok(())
		}

		/// Method: buy_carbon_credit_lot_units
		/// 
		///	- `seller`: T::AccountId - lot seller
		///	- `asset_id`: AssetId - Carbon assets asset id
		///	- `deadline`: T::Moment - expiration time for the lot 
		///	- `amount`: T::Balance - amount of Carbon asset to buy
		/// 
		/// Access: any account having enough EverUSD,
		/// 		for private lot - only account in that lot
		/// 
		/// Buys a specified amount of Carbon asset from specified lot created by 
		/// `create_carbon_credit_lot` call. Lot should not be expired. 
		/// 
		/// Buyer should have enough EverUSD balance. If lot is private 
		/// (`lot.targer_bearer` are set) - only `target_bearer` can buy from that lot. 
		/// After selling other expired seller's lots are purged.
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(5, 3))]
		pub fn buy_carbon_asset_lot_units(
			origin: OriginFor<T>,
			#[pallet::compact] asset_id: AssetId,
			seller: T::AccountId,
			deadline: T::Moment,
			// #[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;
			let now = pallet_timestamp::Pallet::<T>::get();
			// ensure!(deadline >= now, Error::<T, I>::DeadlineIncorrect);

			// first - fix diying if balance low but reserved >0
			// remove expired lot and unreserve balance
			// transfer asset_id from seller reserved to caller balance
			// check if Account(asset_id,seller) is dead -> remove
			// transfer everusd from caller balance to seller balance
			// check if Account(everusd_id,caller) is dead -> remove

			// Account::<T,I>::try_mutate(asset_id, &seller, |data_opt| -> DispatchResult {
			// 	match data_opt {
			// 		None => Err(Error::<T,I>::BalanceLow)?,
			// 		Some(data) => {
			// 			CarbonAssetLotRegistry::<T,I>::try_mutate_exists((asset_id, &seller, deadline), |lot_opt| -> DispatchResult {
			// 				match lot_opt {
			// 					None => Err(Error::<T,I>::BalanceLow)?,
			// 					Some(lot) => {
			// 						// check if everusd is enough
			// 						let everusd_caller = Account::<T,I>::get(EVERUSD_ID, &caller);
			// 						ensure!(everusd_caller.is_some(), Error::<T,I>::BalanceLow);
			// 						ensure!(everusd_caller.unwrap().balance >= lot.total_price, Error::<T,I>::BalanceLow);

			// 						// transfer cabon assets

			// 						Account::<T, I>::try_mutate(asset_id, &caller, |caller_opt| -> DispatchResult {
			// 							match caller_opt {
			// 								Some(ref mut account) => {
			// 									account.balance.saturating_accrue(lot.amount);
			// 								},
			// 								None => {
			// 									*caller_opt = Some(AssetAccountOf::<T, I> {
			// 										balance: lot.amount,
			// 										reserved_balance: Zero::zero(),
			// 										is_frozen: false,
			// 										reason: Self::new_account(&caller, details, None)?,
			// 										extra: T::Extra::default(),
			// 									});
			// 								}
			// 							};
			// 							ensure!(data.reserved_balance >= lot.amount, Error::<T,I>::BalanceLow);
			// 							data.reserved_balance = data.reserved_balance.saturating_sub(lot.amount);
	
			// 							// transfer everusd
			// 							let f = TransferFlags { keep_alive: false, best_effort: false, burn_dust: false };
			// 							Self::do_transfer(EVERUSD_ID, &caller, &seller, lot.total_price, None, f)?;
			// 							Ok(())
			// 						});
			// 						*lot_opt = None;
			// 						Ok(())
			// 					}
			// 				}	
			// 			})?;
			// 			Ok(())
			// 		}
			// 	}
			// })?;

			Ok(())
		}
	}
}
