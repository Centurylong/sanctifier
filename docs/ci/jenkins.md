# Jenkins CI Integration for Sanctifier

Integrate the Sanctifier security suite directly into your enterprise Jenkins pipelines.

## Usage Snippet

```groovy
@Library("sanctifier-shared-library")_

pipeline {
    agent any
    stages {
        stage("Security") {
            steps {
                sanctifierScan targetPath: "./contracts/my-token", failOnSeverity: "HIGH"
            }
        }
    }
}
```
