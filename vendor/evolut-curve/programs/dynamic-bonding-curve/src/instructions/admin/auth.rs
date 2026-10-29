use anchor_lang::prelude::*;

pub mod admin {
    use anchor_lang::{prelude::Pubkey, solana_program::pubkey};

    #[cfg(not(feature = "mainnet"))]
    pub const ADMINS: [Pubkey; 1] = [pubkey!("21Q1v2yW21YFZG7xvguHNDdpc7hPQAnjfWHqo74342TQ")];
    #[cfg(feature = "mainnet")]
    pub const ADMINS: [Pubkey; 1] = [pubkey!("5HF4Vno1pAdY8UdPMqrpAGtziNUX6kFTCH1qMokDPsBP")];
}

pub mod treasury {
    use anchor_lang::{prelude::Pubkey, solana_program::pubkey};

    #[cfg(not(feature = "mainnet"))]
    pub const ID: Pubkey = pubkey!("21Q1v2yW21YFZG7xvguHNDdpc7hPQAnjfWHqo74342TQ");
    #[cfg(feature = "mainnet")]
    pub const ID: Pubkey = pubkey!("ASemhYRLkceP777SAfYVShjdjt481bKw7xUJuaj8u7a4");
}

pub fn assert_eq_admin(admin: Pubkey) -> bool {
    crate::admin::admin::ADMINS
        .iter()
        .any(|predefined_admin| predefined_admin.eq(&admin))
}
