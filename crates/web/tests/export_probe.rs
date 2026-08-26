use opaque_ke::rand::rngs::OsRng;
use opaque_ke::{
    ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters, ServerLogin, ServerLoginParameters, ServerRegistration,
    ServerSetup,
};
use sylvie_core::codec;
use sylvie_core::opaque::{self, Suite};

#[test]
fn export_key_stable() {
    let setup = ServerSetup::<Suite>::new(&mut OsRng);
    let started = ClientRegistration::<Suite>::start(&mut OsRng, b"pw123456").unwrap();
    let response = {
        let s = ServerRegistration::<Suite>::start(&setup, started.message, b"alee").unwrap();
        codec::encode(&s.message.serialize())
    };
    let finished = started
        .state
        .finish(
            &mut OsRng,
            b"pw123456",
            opaque::reg_reply(&codec::decode(&response).unwrap()).unwrap(),
            ClientRegistrationFinishParameters::new(opaque::peer("alee"), None),
        )
        .unwrap();
    let reg_export = finished.export_key.as_slice().to_vec();
    let record = ServerRegistration::<Suite>::finish(finished.message);

    let started = ClientLogin::<Suite>::start(&mut OsRng, b"pw123456").unwrap();
    let reply = {
        let s = ServerLogin::<Suite>::start(
            &mut OsRng,
            &setup,
            Some(record),
            started.message,
            b"alee",
            ServerLoginParameters {
                identifiers: opaque::peer("alee"),
                ..Default::default()
            },
        )
        .unwrap();
        codec::encode(&s.message.serialize())
    };
    let finished = started
        .state
        .finish(
            &mut OsRng,
            b"pw123456",
            opaque::log_reply(&codec::decode(&reply).unwrap()).unwrap(),
            ClientLoginFinishParameters::new(None, opaque::peer("alee"), None),
        )
        .unwrap();
    assert_eq!(reg_export, finished.export_key.as_slice());
}
