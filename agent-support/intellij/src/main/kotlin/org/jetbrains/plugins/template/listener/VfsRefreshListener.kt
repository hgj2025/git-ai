package org.jetbrains.plugins.template.listener

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.openapi.vfs.newvfs.BulkFileListener
import com.intellij.openapi.vfs.newvfs.events.VFileContentChangeEvent
import com.intellij.openapi.vfs.newvfs.events.VFileEvent
import org.jetbrains.plugins.template.model.AgentV1Input
import org.jetbrains.plugins.template.services.GitAiService
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit

internal data class SweepEntry(val relativePath: String, val content: String)

internal fun drainPendingPaths(
    pendingSweepPaths: ConcurrentHashMap<String, MutableSet<String>>,
    workspaceRoot: String
): List<String> {
    val pendingPathsForRoot = pendingSweepPaths[workspaceRoot] ?: return emptyList()
    val pathsToSweep = pendingPathsForRoot.toList()
    if (pathsToSweep.isEmpty()) return emptyList()

    // Remove only the snapshot entries; keep the set in the map so concurrent
    // adders holding a previously-fetched set reference cannot orphan paths.
    pathsToSweep.forEach { pendingPathsForRoot.remove(it) }
    return pathsToSweep
}

internal fun collectSweepEntriesForPaths(
    workspaceRoot: String,
    pathsToSweep: List<String>,
    agentTouchedFiles: ConcurrentHashMap<String, TrackedAgent>,
    now: Long,
    readContent: (String) -> String?,
): Map<String, List<SweepEntry>> {
    val entriesByAgent = mutableMapOf<String, MutableList<SweepEntry>>()

    for (absolutePath in pathsToSweep) {
        val tracked = agentTouchedFiles[absolutePath] ?: continue
        if (tracked.workspaceRoot != workspaceRoot) continue

        if (now - tracked.trackedAt > TrackedAgent.STALE_THRESHOLD_MS) {
            agentTouchedFiles.remove(absolutePath, tracked)
            continue
        }

        val content = readContent(absolutePath) ?: continue
        if (content == tracked.lastCheckpointContent) continue

        if (!agentTouchedFiles.remove(absolutePath, tracked)) continue

        val relativePath = toRelativePath(absolutePath, workspaceRoot)
        entriesByAgent.getOrPut(tracked.agentName) { mutableListOf() }
            .add(SweepEntry(relativePath, content))
    }

    return entriesByAgent
}

/**
 * Listens for VFS refresh events to detect AI agent disk writes on tracked files.
 * Only fires on actual disk changes (isFromRefresh == true), never on in-editor edits.
 *
 * This eliminates false positives from human typing, IDE refactoring, and VCS operations
 * that the DocumentChangeListener's document-level events cannot distinguish.
 */
class VfsRefreshListener(
    private val agentTouchedFiles: ConcurrentHashMap<String, TrackedAgent>,
    private val scheduler: ScheduledExecutorService,
) : BulkFileListener {

    private val logger = Logger.getInstance(VfsRefreshListener::class.java)

    // Sweep checkpoint debounce (5 seconds) - batches VFS refresh events
    private val sweepDebounceMs = 5000L

    // Pending sweep checkpoints per workspace root (debounced)
    private val pendingSweeps = ConcurrentHashMap<String, ScheduledFuture<*>>()

    // Paths with refresh events pending sweep, grouped by workspace root.
    private val pendingSweepPaths = ConcurrentHashMap<String, MutableSet<String>>()

    override fun after(events: List<VFileEvent>) {
        val workspaceRootsToSweep = mutableSetOf<String>()

        for (event in events) {
            if (event !is VFileContentChangeEvent) continue
            if (!event.isFromRefresh) continue
            val tracked = agentTouchedFiles[event.path] ?: continue
            val workspaceRoot = tracked.workspaceRoot
            pendingSweepPaths
                .computeIfAbsent(workspaceRoot) { ConcurrentHashMap.newKeySet() }
                .add(event.path)
            workspaceRootsToSweep.add(workspaceRoot)
        }

        for (root in workspaceRootsToSweep) {
            scheduleSweepCheckpoint(root)
        }
    }

    private fun scheduleSweepCheckpoint(workspaceRoot: String) {
        pendingSweeps[workspaceRoot]?.cancel(false)
        val future = scheduler.schedule({
            executeSweepCheckpoint(workspaceRoot)
        }, sweepDebounceMs, TimeUnit.MILLISECONDS)
        pendingSweeps[workspaceRoot] = future
    }

    private fun executeSweepCheckpoint(workspaceRoot: String) {
        pendingSweeps.remove(workspaceRoot)

        val pathsToSweep = drainPendingPaths(
            pendingSweepPaths = pendingSweepPaths,
            workspaceRoot = workspaceRoot
        )

        if (pathsToSweep.isEmpty()) return

        val now = System.currentTimeMillis()
        val entriesByAgent = collectSweepEntriesForPaths(
            workspaceRoot = workspaceRoot,
            pathsToSweep = pathsToSweep,
            agentTouchedFiles = agentTouchedFiles,
            now = now
        ) { absolutePath ->
            ApplicationManager.getApplication().runReadAction<String?> {
                LocalFileSystem.getInstance().findFileByPath(absolutePath)
                    ?.let { String(it.contentsToByteArray(), Charsets.UTF_8) }
            }
        }

        val service = GitAiService.getInstance()
        for ((agent, entries) in entriesByAgent) {
            val input = AgentV1Input.AiAgent(
                repoWorkingDir = workspaceRoot,
                editedFilepaths = entries.map { it.relativePath },
                agentName = agent,
                conversationId = service.sessionId,
                dirtyFiles = entries.associate { it.relativePath to it.content }
            )

            logger.warn("Triggering sweep checkpoint for $agent on ${entries.size} file(s): ${entries.map { it.relativePath }}")

            service.checkpoint(input, workspaceRoot)
        }
    }
}
