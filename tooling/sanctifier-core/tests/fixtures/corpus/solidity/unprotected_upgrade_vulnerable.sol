// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// EVM mirror of gallery/upgrade_auth_vulnerable.rs.
// Slither: unprotected-upgrade. Aderyn: centralization-risk / unprotected-initializer.
contract UnprotectedUpgradeVulnerable {
    address public implementation;

    // VULN: anyone can repoint the implementation the proxy delegatecalls into,
    // i.e. take over the contract. No access control on the upgrade entrypoint.
    function upgradeTo(address newImplementation) external {
        implementation = newImplementation;
    }

    fallback() external payable {
        address impl = implementation;
        assembly {
            calldatacopy(0, 0, calldatasize())
            let result := delegatecall(gas(), impl, 0, calldatasize(), 0, 0)
            returndatacopy(0, 0, returndatasize())
            switch result
            case 0 { revert(0, returndatasize()) }
            default { return(0, returndatasize()) }
        }
    }

    receive() external payable {}
}
