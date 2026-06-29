// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// EVM mirror of gallery/confused_deputy_vulnerable.rs.
// Slither: tx-origin. Aderyn: arbitrary-from-in-transfer-from (same family:
// the authority check is on the wrong / attacker-influenceable party).
contract ConfusedDeputyVulnerable {
    address public owner;

    constructor() {
        owner = msg.sender;
    }

    // VULN: authenticates tx.origin, not msg.sender. A malicious intermediate
    // contract the owner calls can act as a confused deputy and pass this check.
    function setOwner(address newOwner) external {
        require(tx.origin == owner, "not owner");
        owner = newOwner;
    }
}
