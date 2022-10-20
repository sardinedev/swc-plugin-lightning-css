use std::path::PathBuf;

use swc_core::{
    common::chain,
    ecma::parser::{EsConfig, Syntax, TsConfig},
    ecma::transforms::testing::test_fixture,
    ecma::transforms::{base, testing::FixtureTestConfig},
};

fn es_syntax() -> Syntax {
    Syntax::Es(EsConfig {
        jsx: true,
        ..Default::default()
    })
}

fn ts_syntax() -> Syntax {
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    })
}

#[testing::fixture("tests/fixture/**/input.tsx")]
fn fixture(input: PathBuf) {
    let config = FixtureTestConfig {
        syntax: ts_syntax(),
        ..Default::default()
    };

    test_fixture(input, |_, _| chain!(resolver(),), config)
}
