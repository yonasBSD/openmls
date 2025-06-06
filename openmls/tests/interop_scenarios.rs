use openmls::{
    prelude::*,
    test_utils::test_framework::{
        noop_authentication_service, ActionType, CodecUse, MlsGroupTestSetup,
    },
    treesync::LeafNodeParameters,
};
use openmls_test::openmls_test;

// The following tests correspond to the interop test scenarios detailed here:
// https://github.com/mlswg/mls-implementations/blob/master/test-scenarios.md
// The tests are conducted for every available ciphersuite, but currently only
// using BasicCredentials. We can change the test setup once #134 is fixed.

// # 1:1 join
// A:    Create group
// B->A: KeyPackage
// A->B: Welcome
// ***:  Verify group state
#[openmls_test]
fn one_to_one_join() {
    println!("Testing ciphersuite {ciphersuite:?}");
    let number_of_clients = 2;
    let setup = MlsGroupTestSetup::<Provider>::new(
        MlsGroupCreateConfig::test_default(ciphersuite),
        number_of_clients,
        CodecUse::StructMessages,
    );

    // Create a group with a random creator.
    let group_id = setup
        .create_group(ciphersuite)
        .expect("Error while trying to create group.");
    let mut groups = setup.groups.write().expect("An unexpected error occurred.");
    let group = groups
        .get_mut(&group_id)
        .expect("An unexpected error occurred.");

    let (_, alice_id) = group
        .members()
        .next()
        .expect("An unexpected error occurred.");

    // A vector including bob's id.
    let bob_id = setup
        .random_new_members_for_group(group, 1)
        .expect("An unexpected error occurred.");

    setup
        .add_clients(
            ActionType::Commit,
            group,
            &alice_id,
            bob_id,
            &noop_authentication_service,
        )
        .expect("Error adding Bob");

    // Check that group members agree on a group state.
    setup.check_group_states(group, noop_authentication_service);
}

// # 3-party join
// A: Create group
// B->A: KeyPackage
// A->B: Welcome
// C->A: KeyPackage
// A->B: Add(C), Commit
// A->C: Welcome
// ***:  Verify group state
#[openmls_test]
fn three_party_join() {
    println!("Testing ciphersuite {ciphersuite:?}");

    let number_of_clients = 3;
    let setup = MlsGroupTestSetup::<Provider>::new(
        MlsGroupCreateConfig::test_default(ciphersuite),
        number_of_clients,
        CodecUse::StructMessages,
    );

    // Create a group with a random creator.
    let group_id = setup
        .create_group(ciphersuite)
        .expect("Error while trying to create group.");
    let mut groups = setup.groups.write().expect("An unexpected error occurred.");
    let group = groups
        .get_mut(&group_id)
        .expect("An unexpected error occurred.");

    let (_, alice_id) = group
        .members()
        .next()
        .expect("An unexpected error occurred.");

    // A vector including Bob's id.
    let bob_id = setup
        .random_new_members_for_group(group, 1)
        .expect("An unexpected error occurred.");

    // Create the add commit and deliver the welcome.
    setup
        .add_clients(
            ActionType::Commit,
            group,
            &alice_id,
            bob_id,
            &noop_authentication_service,
        )
        .expect("Error adding Bob");

    // A vector including Charly's id.
    let charly_id = setup
        .random_new_members_for_group(group, 1)
        .expect("An unexpected error occurred.");

    setup
        .add_clients(
            ActionType::Commit,
            group,
            &alice_id,
            charly_id,
            &noop_authentication_service,
        )
        .expect("Error adding Charly");

    // Check that group members agree on a group state.
    setup.check_group_states(group, noop_authentication_service);
}

// # Multiple joins at once
// A:    Create group
// B->A: KeyPackage
// C->A: KeyPackage
// A->B: Welcome
// A->C: Welcome
// ***:  Verify group state
#[openmls_test]
fn multiple_joins() {
    println!("Testing ciphersuite {ciphersuite:?}");

    let number_of_clients = 3;
    let setup = MlsGroupTestSetup::<Provider>::new(
        MlsGroupCreateConfig::test_default(ciphersuite),
        number_of_clients,
        CodecUse::StructMessages,
    );

    // Create a group with a random creator.
    let group_id = setup
        .create_group(ciphersuite)
        .expect("Error while trying to create group.");
    let mut groups = setup.groups.write().expect("An unexpected error occurred.");
    let group = groups
        .get_mut(&group_id)
        .expect("An unexpected error occurred.");

    let (_, alice_id) = group
        .members()
        .next()
        .expect("An unexpected error occurred.");

    // A vector including Bob's and Charly's id.
    let bob_charly_id = setup
        .random_new_members_for_group(group, 2)
        .expect("An unexpected error occurred.");

    // Create the add commit and deliver the welcome.
    setup
        .add_clients(
            ActionType::Commit,
            group,
            &alice_id,
            bob_charly_id,
            &noop_authentication_service,
        )
        .expect("Error adding Bob and Charly");

    // Check that group members agree on a group state.
    setup.check_group_states(group, noop_authentication_service);
}

// # Update
// A:    Create group
// B->A: KeyPackage
// A->B: Welcome
// A->B: Update, Commit
// ***:  Verify group state
#[openmls_test]
fn update() {
    println!("Testing ciphersuite {ciphersuite:?}");

    let number_of_clients = 2;
    let setup = MlsGroupTestSetup::<Provider>::new(
        MlsGroupCreateConfig::test_default(ciphersuite),
        number_of_clients,
        CodecUse::StructMessages,
    );

    // Create a group with two members. Includes the process of adding Bob.
    let group_id = setup
        .create_random_group(2, ciphersuite, noop_authentication_service)
        .expect("Error while trying to create group.");
    let mut groups = setup.groups.write().expect("An unexpected error occurred.");
    let group = groups
        .get_mut(&group_id)
        .expect("An unexpected error occurred.");

    let (_, alice_id) = group
        .members()
        .next()
        .expect("An unexpected error occurred.");

    // Let Alice create an update with a self-generated KeyPackageBundle.
    setup
        .self_update(
            ActionType::Commit,
            group,
            &alice_id,
            LeafNodeParameters::default(),
            &noop_authentication_service,
        )
        .expect("Error self-updating.");

    // Check that group members agree on a group state.
    setup.check_group_states(group, noop_authentication_service);
}

// # Remove
// A:    Create group
// B->A: KeyPackage
// C->A: KeyPackage
// A->B: Welcome
// A->C: Welcome
// A->B: Remove(B), Commit
// ***:  Verify group state
#[openmls_test]
fn remove() {
    println!("Testing ciphersuite {ciphersuite:?}");

    let number_of_clients = 2;
    let setup = MlsGroupTestSetup::<Provider>::new(
        MlsGroupCreateConfig::test_default(ciphersuite),
        number_of_clients,
        CodecUse::StructMessages,
    );

    // Create a group with two members. Includes the process of adding Bob.
    let group_id = setup
        .create_random_group(2, ciphersuite, noop_authentication_service)
        .expect("Error while trying to create group.");
    let mut groups = setup.groups.write().expect("An unexpected error occurred.");
    let group = groups
        .get_mut(&group_id)
        .expect("An unexpected error occurred.");

    let (_, alice_id) = group
        .members()
        .next()
        .expect("An unexpected error occurred.");
    let (bob_index, _) = group
        .members()
        .last()
        .expect("An unexpected error occurred.");

    // Have alice remove Bob.
    setup
        .remove_clients(
            ActionType::Commit,
            group,
            &alice_id,
            &[LeafNodeIndex::new(bob_index)],
            noop_authentication_service,
        )
        .expect("Error removing Bob from the group.");

    // Check that group members agree on a group state.
    setup.check_group_states(group, noop_authentication_service);
}

// # Large Group, Full Lifecycle
// * Create group
// * Group creator adds the first M members
// * Until group size reaches N members, a randomly-chosen group member adds a
//   new member
// * All members update
// * While the group size is >1, a randomly-chosen group member removes a
//   randomly-chosen other group member
#[openmls_test]
fn large_group_lifecycle() {
    println!("Testing ciphersuite {ciphersuite:?}");

    // "Large" is 20 for now.
    let number_of_clients = 20;
    let setup = MlsGroupTestSetup::<Provider>::new(
        MlsGroupCreateConfig::test_default(ciphersuite),
        number_of_clients,
        CodecUse::StructMessages,
    );

    // Create a group with all available clients. The process includes creating
    // a one-person group and then adding new members in bunches of up to 5,
    // each bunch by a random group member.
    let group_id = setup
        .create_random_group(number_of_clients, ciphersuite, noop_authentication_service)
        .expect("Error while trying to create group.");
    let mut groups = setup.groups.write().expect("An unexpected error occurred.");
    let group = groups
        .get_mut(&group_id)
        .expect("An unexpected error occurred.");

    let mut group_members = group.members().collect::<Vec<(u32, Vec<u8>)>>();

    // Have each member in turn update. In between each update, messages are
    // delivered to each member.
    for (_, member_id) in &group_members {
        setup
            .self_update(
                ActionType::Commit,
                group,
                member_id,
                LeafNodeParameters::default(),
                &noop_authentication_service,
            )
            .expect("Error while updating group.")
    }

    while group_members.len() > 1 {
        let remover_id = group.random_group_member();
        let mut target_id = group.random_group_member();
        // Get a random member until it's not the one doing the remove operation.
        while remover_id == target_id {
            target_id = group.random_group_member();
        }
        setup
            .remove_clients(
                ActionType::Commit,
                group,
                &remover_id.1,
                &[LeafNodeIndex::new(target_id.0)],
                noop_authentication_service,
            )
            .expect("Error while removing group member.");
        group_members = group.members().collect::<Vec<(u32, Vec<u8>)>>();
        setup.check_group_states(group, noop_authentication_service);
    }

    // Check that group members agree on a group state.
    setup.check_group_states(group, noop_authentication_service);
}
