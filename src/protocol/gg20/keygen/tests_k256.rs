use super::*;
use crate::protocol::gg20::{
    tests::keygen::{TEST_CASES, TEST_CASES_INVALID},
    vss_k256,
};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn basic_correctness() {
    for &(share_count, threshold) in TEST_CASES.iter() {
        execute_keygen(share_count, threshold);
    }

    // TODO refactor so there's no need to catch panics
    // silence terminal output from catch_unwind https://stackoverflow.com/questions/35559267/suppress-panic-output-in-rust-when-using-paniccatch-unwind/35559417#35559417
    std::panic::set_hook(Box::new(|_| {}));
    for &(share_count, threshold) in TEST_CASES_INVALID.iter() {
        assert!(std::panic::catch_unwind(|| execute_keygen(share_count, threshold)).is_err());
    }
}

pub(crate) fn execute_keygen(share_count: usize, threshold: usize) -> Vec<SecretKeyShare> {
    let mut parties: Vec<Keygen> = (0..share_count)
        .map(|i| Keygen::new(share_count, threshold, i).unwrap())
        .collect();

    // execute round 1 all parties and store their outputs
    let mut all_r1_bcasts = FillVec::with_len(share_count);
    for (i, party) in parties.iter_mut().enumerate() {
        let (state, bcast) = party.r1();
        party.r1state = Some(state);
        party.status = Status::R1;
        all_r1_bcasts.insert(i, bcast).unwrap();
    }
    let all_r1_bcasts = all_r1_bcasts; // make read-only

    // deliver round 1 msgs
    for party in parties.iter_mut() {
        party.in_r1bcasts = all_r1_bcasts.clone();
    }

    // save each u for later tests
    let all_u_secrets: Vec<k256::Scalar> = parties
        .iter()
        .map(|p| {
            p.r1state
                .as_ref()
                .unwrap()
                .my_u_i_vss_k256
                .get_secret()
                .clone()
        })
        .collect();

    // execute round 2 all parties and store their outputs
    let mut all_r2_bcasts = FillVec::with_len(share_count);
    let mut all_r2_p2ps = Vec::with_capacity(share_count);
    for (i, party) in parties.iter_mut().enumerate() {
        let (state, bcast, p2ps) = party.r2();
        party.r2state = Some(state);
        party.status = Status::R2;
        all_r2_bcasts.insert(i, bcast).unwrap();
        all_r2_p2ps.push(p2ps);
    }
    let all_r2_bcasts = all_r2_bcasts; // make read-only
    let all_r2_p2ps = all_r2_p2ps; // make read-only

    // deliver round 2 msgs
    for party in parties.iter_mut() {
        party.in_all_r2p2ps = all_r2_p2ps.clone();
        party.in_r2bcasts = all_r2_bcasts.clone();
    }

    // execute round 3 all parties and store their outputs
    let mut all_r3_bcasts = FillVec::with_len(share_count);
    for (i, party) in parties.iter_mut().enumerate() {
        match party.r3() {
            r3::Output::Success { state, out_bcast } => {
                party.r3state = Some(state);
                party.status = Status::R3;
                all_r3_bcasts.insert(i, out_bcast).unwrap();
            }
            _ => {
                panic!("r3 party {} expect success got failure", party.my_index);
            }
        }
    }
    let all_r3_bcasts = all_r3_bcasts; // make read-only

    // deliver round 3 msgs
    for party in parties.iter_mut() {
        party.in_r3bcasts = all_r3_bcasts.clone();
    }

    // execute round 4 all parties and store their outputs
    let mut all_secret_key_shares = Vec::with_capacity(share_count);
    for party in parties.iter_mut() {
        match party.r4() {
            r4::Output::Success { key_share } => {
                all_secret_key_shares.push(key_share);
            }
            r4::Output::Fail { criminals } => {
                panic!(
                    "r4 party {} expect success got failure with criminals: {:?}",
                    party.my_index, criminals
                );
            }
        }
        party.status = Status::Done;
    }
    let all_secret_key_shares = all_secret_key_shares; // make read-only

    // test: reconstruct the secret key in two ways:
    // 1. from all the u secrets of round 1
    // 2. from the first t+1 shares
    let secret_key_sum_u = all_u_secrets
        .iter()
        .fold(k256::Scalar::zero(), |acc, &x| acc + x);

    let all_shares: Vec<vss_k256::Share> = all_secret_key_shares
        .iter()
        .map(|k| vss_k256::Share::from_scalar(*k.share.my_x_i_k256.unwrap(), k.share.my_index))
        .collect();
    let secret_key_recovered = vss_k256::recover_secret(&all_shares, threshold);

    assert_eq!(secret_key_recovered, secret_key_sum_u);

    // test: verify that the reconstructed secret key yields the public key everyone deduced
    for secret_key_share in all_secret_key_shares.iter() {
        let test_pubkey = k256::ProjectivePoint::generator() * secret_key_recovered;
        assert_eq!(&test_pubkey, secret_key_share.group.y_k256.unwrap());
    }

    // test: everyone computed everyone else's public key share correctly
    for (i, secret_key_share) in all_secret_key_shares.iter().enumerate() {
        for (j, other_secret_key_share) in all_secret_key_shares.iter().enumerate() {
            assert_eq!(
                *secret_key_share.group.all_y_i_k256[j].unwrap(),
                k256::ProjectivePoint::generator()
                    * other_secret_key_share.share.my_x_i_k256.unwrap(),
                "party {} got party {} key wrong",
                i,
                j
            );
        }
    }

    all_secret_key_shares
}