#![deny(clippy::all)]
#![warn(clippy::nursery)]

use std::collections::BTreeSet;
use std::path::Path;
use vrc_udon_methods_extractor::{discover_sdk_trees, extract_tree};

#[test]
#[ignore = "requires local world-sdk/trees populated by world-sdk/fetch-all.sh"]
fn extracts_mangled_symbols_from_every_world_sdk_tree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../world-sdk/trees");
    let trees = discover_sdk_trees(&root).expect("discover SDK trees");
    assert!(!trees.is_empty(), "expected at least one SDK tree");

    for tree in trees {
        let records = extract_tree(tree.version.clone(), &tree.root)
            .unwrap_or_else(|error| panic!("failed to extract {}: {error:#}", tree.version));
        assert!(
            !records.is_empty(),
            "expected non-empty extraction for {}",
            tree.version
        );

        let mut seen = BTreeSet::new();
        for record in &records {
            assert_eq!(record.version, tree.version);
            assert!(
                record.symbol.contains(".__"),
                "symbol should retain mangled fullName: {}",
                record.symbol
            );
            assert!(
                seen.insert(record.symbol.clone()),
                "duplicate symbol in {}: {}",
                tree.version,
                record.symbol
            );
        }
    }
}

#[test]
#[ignore = "requires local world-sdk/trees populated by world-sdk/fetch-all.sh"]
fn restores_fqcn_from_extern_wrapper_method_bodies() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../world-sdk/trees/3.10.3");
    let records = extract_tree("3.10.3", root).expect("extract 3.10.3");

    let player = records
        .iter()
        .find(|record| record.declaring_type == "VRCSDKBaseVRCPlayerApi")
        .expect("VRCPlayerApi extern wrapper");
    assert_eq!(
        player.declaring_type_fqcn.as_deref(),
        Some("VRC.SDKBase.VRCPlayerApi")
    );

    let teleport = records
        .iter()
        .find(|record| {
            record.symbol
                == "VRCSDKBaseVRCPlayerApi.__TeleportTo__UnityEngineVector3_UnityEngineQuaternion_VRCSDKBaseVRC_SceneDescriptorSpawnOrientation__SystemVoid"
        })
        .expect("TeleportTo(Vector3, Quaternion, SpawnOrientation)");
    assert_eq!(
        teleport.parameters[0].fqcn.as_deref(),
        Some("UnityEngine.Vector3")
    );
    assert_eq!(
        teleport.parameters[1].fqcn.as_deref(),
        Some("UnityEngine.Quaternion")
    );
    assert_eq!(
        teleport.parameters[2].fqcn.as_deref(),
        Some("VRC.SDKBase.VRC_SceneDescriptor/SpawnOrientation")
    );

    let get_type = records
        .iter()
        .find(|record| record.symbol == "VRCSDKBaseVRCPlayerApi.__GetType__SystemType")
        .expect("GetType() -> Type");
    assert_eq!(
        get_type
            .return_type
            .as_ref()
            .and_then(|ty| ty.fqcn.as_deref()),
        Some("System.Type")
    );

    let transform = records
        .iter()
        .find(|record| {
            record.symbol == "CinemachineCinemachineDollyCart.__get_transform__UnityEngineTransform"
        })
        .expect("CinemachineDollyCart.get_transform");
    assert_eq!(
        transform.declaring_type_fqcn.as_deref(),
        Some("Cinemachine.CinemachineDollyCart")
    );
    assert_eq!(
        transform
            .return_type
            .as_ref()
            .and_then(|ty| ty.fqcn.as_deref()),
        Some("UnityEngine.Transform")
    );

    let list_t = records
        .iter()
        .find(|record| {
            record.symbol
                == "CinemachineCinemachineDollyCart.__GetComponentsInChildren__ListT__SystemVoid"
        })
        .expect("GetComponentsInChildren(List<T>)");
    assert_eq!(
        list_t.parameters[0].fqcn.as_deref(),
        Some("System.Collections.Generic.List<UnityEngine.Object>")
    );

    let contact_sender = records
        .iter()
        .find(|record| {
            record.symbol
                == "VRCDynamicsContactEnterInfo.__get_contactSender__VRCDynamicsContactSenderProxy"
        })
        .expect("ContactEnterInfo.contactSender");
    assert_eq!(
        contact_sender
            .return_type
            .as_ref()
            .and_then(|ty| ty.fqcn.as_deref()),
        Some("VRC.Dynamics.ContactSenderProxy")
    );
}
