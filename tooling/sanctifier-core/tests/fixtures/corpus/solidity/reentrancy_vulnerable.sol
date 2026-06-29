// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// EVM mirror of gallery/reentrancy_vulnerable.rs.
// Slither: reentrancy-eth. Aderyn: state-change-after-external-call.
contract ReentrancyVulnerable {
    mapping(address => uint256) public balances;

    function deposit() external payable {
        balances[msg.sender] += msg.value;
    }

    // VULN: external call (interaction) happens BEFORE the balance update (effect),
    // so a re-entrant call observes the stale balance and drains the contract.
    function withdraw(uint256 amount) external {
        require(balances[msg.sender] >= amount, "insufficient");
        (bool ok, ) = msg.sender.call{value: amount}("");
        require(ok, "transfer failed");
        balances[msg.sender] -= amount;
    }
}
