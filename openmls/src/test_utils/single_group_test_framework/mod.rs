use openmls_basic_credential::SignatureKeyPair;
use openmls_traits::{signatures::Signer, types::SignatureScheme};
pub use openmls_traits::{
    storage::StorageProvider as StorageProviderTrait,
    types::{Ciphersuite, HpkeKeyPair},
    OpenMlsProvider,
};

pub use crate::utils::*;
use crate::{
    credentials::CredentialWithKey,
    key_packages::KeyPackageBuilder,
    prelude::{commit_builder::*, *},
};

mod errors;
use errors::*;

use std::collections::HashMap;

// type alias for &'static str
type Name = &'static str;

// TODO: only define this once
pub(crate) fn generate_credential(
    identity: Vec<u8>,
    signature_algorithm: SignatureScheme,
    provider: &impl crate::storage::OpenMlsProvider,
) -> (CredentialWithKey, SignatureKeyPair) {
    let credential = BasicCredential::new(identity);
    let signature_keys = SignatureKeyPair::new(signature_algorithm).unwrap();
    signature_keys.store(provider.storage()).unwrap();

    (
        CredentialWithKey {
            credential: credential.into(),
            signature_key: signature_keys.to_public_vec().into(),
        },
        signature_keys,
    )
}

// TODO: only define this once
pub(crate) fn generate_key_package(
    ciphersuite: Ciphersuite,
    credential_with_key: CredentialWithKey,
    extensions: Extensions,
    provider: &impl crate::storage::OpenMlsProvider,
    signer: &impl Signer,
) -> KeyPackageBundle {
    KeyPackage::builder()
        .key_package_extensions(extensions)
        .build(ciphersuite, provider, signer, credential_with_key)
        .unwrap()
}

// Struct representing a party's global state
pub struct CorePartyState<Provider> {
    name: Name,
    provider: Provider,
}

impl<Provider: Default> CorePartyState<Provider> {
    pub fn new(name: Name) -> Self {
        Self {
            name,
            provider: Provider::default(),
        }
    }
}

// Struct representing a party's state before joining a group
pub struct PreGroupPartyState<'a, Provider> {
    credential_with_key: CredentialWithKey,
    // TODO: regenerate?
    key_package_bundle: KeyPackageBundle,
    signer: SignatureKeyPair,
    core_state: &'a CorePartyState<Provider>,
}

impl<Provider: OpenMlsProvider> CorePartyState<Provider> {
    // Generates the pre-group state for a `CorePartyState`
    pub fn generate_pre_group(&self, ciphersuite: Ciphersuite) -> PreGroupPartyState<'_, Provider> {
        let (credential_with_key, signer) = generate_credential(
            self.name.into(),
            ciphersuite.signature_algorithm(),
            &self.provider,
        );

        let key_package_bundle = generate_key_package(
            ciphersuite,
            credential_with_key.clone(),
            Extensions::default(), // TODO: provide as argument?
            &self.provider,
            &signer,
        );

        PreGroupPartyState {
            credential_with_key,
            key_package_bundle,
            signer,
            core_state: self,
        }
    }
}

// Represents a group member's `MlsGroup` instance and pre-group state
pub struct MemberState<'a, Provider> {
    party: PreGroupPartyState<'a, Provider>,
    group: MlsGroup,
}

impl<Provider: OpenMlsProvider> MemberState<'_, Provider> {
    // Deliver_and_apply a message to this member's `MlsGroup`
    pub fn deliver_and_apply(
        &mut self,
        message: MlsMessageIn,
    ) -> Result<
        (),
        TestError<
            <<Provider as OpenMlsProvider>::StorageProvider as StorageProviderTrait<1>>::Error,
        >,
    > {
        let message = message.try_into_protocol_message()?;

        // process message
        let processed_message = self
            .group
            .process_message(&self.party.core_state.provider, message)?;

        match processed_message.into_content() {
            ProcessedMessageContent::ApplicationMessage(_) => todo!(),
            ProcessedMessageContent::ProposalMessage(_) => todo!(),
            ProcessedMessageContent::ExternalJoinProposalMessage(_) => todo!(),
            ProcessedMessageContent::StagedCommitMessage(m) => self
                .group
                .merge_staged_commit(&self.party.core_state.provider, *m)?,
        };

        Ok(())
    }
}

impl<'commit_builder, 'b: 'commit_builder, 'a: 'b, Provider> MemberState<'a, Provider>
where
    Provider: openmls_traits::OpenMlsProvider,
{
    // Build and stage a commit, using the provided closure to add proposals
    pub fn build_commit_and_stage(
        &'b mut self,
        f: impl FnOnce(
            CommitBuilder<'commit_builder, Initial>,
        ) -> CommitBuilder<'commit_builder, Initial>,
        // XXX: is there a better way to express this type constraint?
    ) -> Result<
        CommitMessageBundle,
        TestError<
            <<Provider as OpenMlsProvider>::StorageProvider as StorageProviderTrait<1>>::Error,
        >,
    > {
        let commit_builder = f(self.group.commit_builder());

        let provider = &self.party.core_state.provider;

        // TODO: most of the steps here cannot be done via the closure (yet)
        let bundle = commit_builder
            .load_psks(provider.storage())?
            .build(
                provider.rand(),
                provider.crypto(),
                &self.party.signer,
                |_| true,
            )?
            .stage_commit(provider)?;

        Ok(bundle)
    }
}

impl<'a, Provider: OpenMlsProvider> MemberState<'a, Provider> {
    // Create a `MemberState` from a `PreGroupPartyState`. This creates a new `MlsGroup` with one
    // member
    // TODO: builder pattern?
    // - GroupId
    // - MlsGroupCreateConfig
    pub fn create_from_pre_group(
        party: PreGroupPartyState<'a, Provider>,
        mls_group_create_config: MlsGroupCreateConfig,
    ) -> Result<
        Self,
        TestError<
            <<Provider as OpenMlsProvider>::StorageProvider as StorageProviderTrait<1>>::Error,
        >,
    > {
        // initialize MlsGroup
        let group = MlsGroup::new(
            &party.core_state.provider,
            &party.signer,
            &mls_group_create_config,
            party.credential_with_key.clone(),
        )?;

        Ok(Self { party, group })
    }
    // Create a `MemberState` from a `Welcome`, which creates a new `MlsGroup` using a `Welcome`
    // invitation from an existing group
    pub fn join_from_pre_group(
        party: PreGroupPartyState<'a, Provider>,
        mls_group_join_config: MlsGroupJoinConfig,
        welcome: Welcome,
        tree: Option<RatchetTreeIn>,
    ) -> Result<
        Self,
        TestError<
            <<Provider as OpenMlsProvider>::StorageProvider as StorageProviderTrait<1>>::Error,
        >,
    > {
        let staged_join = StagedWelcome::new_from_welcome(
            &party.core_state.provider,
            &mls_group_join_config,
            welcome,
            tree,
        )?;

        let group = staged_join.into_group(&party.core_state.provider)?;

        Ok(Self { party, group })
    }
}

// All of the state for a group and its members
pub struct GroupState<'a, Provider> {
    // TODO: GroupId
    members: HashMap<Name, MemberState<'a, Provider>>,
}

impl<'b, 'a: 'b, Provider: OpenMlsProvider> GroupState<'a, Provider> {
    // Create a new `GroupState` from a single party
    pub fn new_from_party(
        pre_group_state: PreGroupPartyState<'a, Provider>,
        mls_group_create_config: MlsGroupCreateConfig,
    ) -> Result<
        Self,
        TestError<
            <<Provider as OpenMlsProvider>::StorageProvider as StorageProviderTrait<1>>::Error,
        >,
    > {
        let mut members = HashMap::new();

        let name = pre_group_state.core_state.name;
        let member_state =
            MemberState::create_from_pre_group(pre_group_state, mls_group_create_config)?;

        members.insert(name, member_state);

        Ok(Self { members })
    }

    // Get mutable references to all `MemberState`s as a fixed-size array
    pub fn groups_mut<const N: usize>(&mut self) -> [&mut MemberState<'a, Provider>; N] {
        let member_states: Vec<&mut MemberState<'a, Provider>> =
            self.members.values_mut().collect();

        match member_states.try_into() {
            Ok(states) => states,
            Err(_) => panic!("Could not construct array of length {N} from members"),
        }
    }

    // Deliver_and_apply a message to all parties
    pub fn deliver_and_apply(
        &'b mut self,
        message: MlsMessageIn,
    ) -> Result<
        (),
        TestError<
            <<Provider as OpenMlsProvider>::StorageProvider as StorageProviderTrait<1>>::Error,
        >,
    > {
        self.deliver_and_apply_if(message, |_| true)
    }
    // Deliver_and_apply a message to all parties if a provided condition is met
    pub fn deliver_and_apply_if(
        &'b mut self,
        message: MlsMessageIn,
        condition: impl Fn(&MemberState<'a, Provider>) -> bool,
    ) -> Result<
        (),
        TestError<
            <<Provider as OpenMlsProvider>::StorageProvider as StorageProviderTrait<1>>::Error,
        >,
    > {
        self.members
            .values_mut()
            .filter(|member| condition(member))
            .try_for_each(|member| member.deliver_and_apply(message.clone()))?;

        Ok(())
    }

    // Deliver_and_apply a welcome to a single party, and initialize a group for that party
    pub fn deliver_and_apply_welcome(
        &'b mut self,
        recipient: PreGroupPartyState<'a, Provider>,
        mls_group_join_config: MlsGroupJoinConfig,
        welcome: Welcome,
        tree: Option<RatchetTreeIn>,
    ) -> Result<
        (),
        TestError<
            <<Provider as OpenMlsProvider>::StorageProvider as StorageProviderTrait<1>>::Error,
        >,
    > {
        // create new group
        let name = recipient.core_state.name;

        let member_state =
            MemberState::join_from_pre_group(recipient, mls_group_join_config, welcome, tree)?;

        // insert after success
        self.members.insert(name, member_state);

        Ok(())
    }

    // Drop a member from the internal hashmap. This does not delete the member from any
    // `MlsGroup`
    pub fn untrack_member(&mut self, name: Name) {
        let _ = self.members.remove(&name);
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use openmls_test::openmls_test;

    #[openmls_test]
    pub fn simple_example() {
        let alice_party = CorePartyState::<Provider>::new("alice");
        let bob_party = CorePartyState::<Provider>::new("bob");

        let alice_pre_group = alice_party.generate_pre_group(ciphersuite);
        let bob_pre_group = bob_party.generate_pre_group(ciphersuite);

        // Get the key package for Bob
        // TODO: should key package be regenerated each time?
        let bob_key_package = bob_pre_group.key_package_bundle.key_package.clone();

        // Create config
        let mls_group_create_config = MlsGroupCreateConfig::builder()
            .ciphersuite(ciphersuite)
            .use_ratchet_tree_extension(true)
            .build();

        // Join config
        let mls_group_join_config = mls_group_create_config.join_config().clone();

        // Initialize the group state
        let mut group_state =
            GroupState::new_from_party(alice_pre_group, mls_group_create_config).unwrap();

        // Get a mutable reference to Alice's group representation
        let [alice] = group_state.groups_mut();

        // Build a commit with a single add proposal
        let bundle = alice
            .build_commit_and_stage(move |builder| {
                let add_proposal = Proposal::Add(AddProposal {
                    key_package: bob_key_package,
                });

                // ...add more proposals here...

                builder
                    .consume_proposal_store(false)
                    .add_proposal(add_proposal)
            })
            .expect("Could not stage commit");

        // Deliver_and_apply to all members but Alice
        group_state
            .deliver_and_apply_if(bundle.commit().clone().into(), |member| {
                member.party.core_state.name != "alice"
            })
            .expect("Error deliver_and_applying commit");

        // Deliver_and_apply welcome to Bob
        let welcome = bundle.welcome().unwrap().clone();
        group_state
            .deliver_and_apply_welcome(bob_pre_group, mls_group_join_config, welcome, None)
            .expect("Error deliver_and_applying welcome");
    }
}
