import java.io.File
import javax.inject.Inject
import org.apache.tools.ant.taskdefs.condition.Os
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.logging.LogLevel
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations

// Gradle 9 removed Project.exec. Constructor injection is the supported seam;
// `tauri android init` regenerates this file and can undo it.
open class BuildTask @Inject constructor(private val execOperations: ExecOperations) :
    DefaultTask() {
    @Input
    var rootDirRel: String? = null
    @Input
    var target: String? = null
    @Input
    var release: Boolean? = null

    @TaskAction
    fun assemble() {
        // `tauri android init` bakes this pair from how the CLI was invoked when
        // it ran: argv[0] and $npm_execpath. Generated from a bare
        // `node .../tauri.js`, it comes out as `node` + `["tauri", ...]`, which
        // makes Gradle run `node tauri …` in src-tauri and die on a missing
        // module. `npm` + `["run", "--", "tauri", …]` is what the CLI emits
        // under `npm run tauri --`, which is how every build here starts, and
        // `npm run` walks up to crates/copypaste-ui for the package.json.
        val executable = """npm""";
        try {
            runTauriCli(executable)
        } catch (e: Exception) {
            if (Os.isFamily(Os.FAMILY_WINDOWS)) {
                // Try different Windows-specific extensions
                val fallbacks = listOf(
                    "$executable.exe",
                    "$executable.cmd",
                    "$executable.bat",
                )
                
                var lastException: Exception = e
                for (fallback in fallbacks) {
                    try {
                        runTauriCli(fallback)
                        return
                    } catch (fallbackException: Exception) {
                        lastException = fallbackException
                    }
                }
                throw lastException
            } else {
                throw e;
            }
        }
    }

    fun runTauriCli(executable: String) {
        val rootDirRel = rootDirRel ?: throw GradleException("rootDirRel cannot be null")
        val target = target ?: throw GradleException("target cannot be null")
        val release = release ?: throw GradleException("release cannot be null")
        val args = listOf("run", "--", "tauri", "android", "android-studio-script");
        val rustWebViewExtension = File(
            project.projectDir,
            "src/main/rust-webview-accessibility.kt.inc",
        ).readText()

        execOperations.exec {
            workingDir(File(project.projectDir, rootDirRel))
            executable(executable)
            args(args)
            environment("WRY_RUSTWEBVIEW_CLASS_EXTENSION", rustWebViewExtension)
            if (project.logger.isEnabled(LogLevel.DEBUG)) {
                args("-vv")
            } else if (project.logger.isEnabled(LogLevel.INFO)) {
                args("-v")
            }
            if (release) {
                args("--release")
            }
            args(listOf("--target", target))
        }.assertNormalExitValue()
    }
}
