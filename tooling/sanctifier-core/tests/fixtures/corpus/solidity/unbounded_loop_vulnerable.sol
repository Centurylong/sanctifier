// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// EVM mirror of gallery/unbounded_loop_vulnerable.rs.
// Slither: calls-loop. Aderyn: costly-operations-inside-loops.
contract UnboundedLoopVulnerable {
    address[] public recipients;

    function enroll() external {
        recipients.push(msg.sender);
    }

    // VULN: external call inside a loop over a caller-growable array. One reverting
    // (or gas-griefing) recipient bricks the whole distribution (DoS).
    function distribute() external {
        for (uint256 i = 0; i < recipients.length; i++) {
            (bool ok, ) = recipients[i].call{value: 1 ether}("");
            require(ok, "transfer failed");
        }
    }
}
