use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn prove_renders_a_human_readable_minimized_counterexample() {
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.args(["prove", "--invariant", "no_unauthorized_mint", "--no-save"]);

    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("Counterexample"))
        .stdout(predicate::str::contains("Violated assertion: require_auth(&admin) before mint"))
        .stdout(predicate::str::contains("Inputs:"))
        .stdout(predicate::str::contains("- caller_id: 0"))
        .stdout(predicate::str::contains("- admin_id: 1"))
        .stdout(predicate::str::contains("- old_supply: 0"))
        .stdout(predicate::str::contains("- mint_amount: 1"))
        .stdout(predicate::str::contains("Trace: mint(caller=0, admin=1, amount=1)"));
}
