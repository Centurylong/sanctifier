/**
 * Jenkins Shared Library Step for Sanctifier Security Scanner
 */
def call(Map config = [:]) {
    def targetPath = config.get("targetPath", "./contracts")
    def failOnSeverity = config.get("failOnSeverity", "HIGH")

    stage("Sanctifier Security Scan") {
        echo "Running Sanctifier static analysis on ${targetPath}..."
        sh "sanctifier analyze ${targetPath} --format json > sanctifier-report.json"
        archiveArtifacts artifacts: "sanctifier-report.json", fingerprint: true
    }
}
