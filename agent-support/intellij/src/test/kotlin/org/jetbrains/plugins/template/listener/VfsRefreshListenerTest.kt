package org.jetbrains.plugins.template.listener

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import java.util.concurrent.ConcurrentHashMap
import org.junit.Test

class VfsRefreshListenerTest {

    @Test
    fun drainPendingPathsDoesNotOrphanPathsAddedViaCapturedSetReference() {
        val workspaceRoot = "/repo"
        val pendingSweepPaths = ConcurrentHashMap<String, MutableSet<String>>()
        val capturedSet = pendingSweepPaths.computeIfAbsent(workspaceRoot) { ConcurrentHashMap.newKeySet() }

        capturedSet.add("$workspaceRoot/first.txt")
        val firstDrain = drainPendingPaths(pendingSweepPaths, workspaceRoot)
        assertEquals(setOf("$workspaceRoot/first.txt"), firstDrain.toSet())

        // Simulate an `after()` thread that fetched the set reference before drain.
        capturedSet.add("$workspaceRoot/second.txt")
        val secondDrain = drainPendingPaths(pendingSweepPaths, workspaceRoot)
        assertEquals(setOf("$workspaceRoot/second.txt"), secondDrain.toSet())
    }

    @Test
    fun sweepOnlyProcessesPathsThatHadRefreshEvents() {
        val workspaceRoot = "/repo"
        val refreshedPath = "$workspaceRoot/refreshed.txt"
        val unrelatedPath = "$workspaceRoot/unrelated.txt"

        val refreshedTracked = TrackedAgent(
            agentName = "copilot",
            workspaceRoot = workspaceRoot,
            lastCheckpointContent = "before-refresh",
            trackedAt = 1_000L
        )
        val unrelatedTracked = TrackedAgent(
            agentName = "copilot",
            workspaceRoot = workspaceRoot,
            lastCheckpointContent = "before-manual-edit",
            trackedAt = 1_000L
        )

        val trackedFiles = ConcurrentHashMap<String, TrackedAgent>().apply {
            put(refreshedPath, refreshedTracked)
            put(unrelatedPath, unrelatedTracked)
        }

        val readPaths = mutableListOf<String>()

        val entriesByAgent = collectSweepEntriesForPaths(
            workspaceRoot = workspaceRoot,
            pathsToSweep = listOf(refreshedPath),
            agentTouchedFiles = trackedFiles,
            now = 2_000L
        ) { path ->
            readPaths.add(path)
            when (path) {
                refreshedPath -> "after-refresh"
                unrelatedPath -> "after-manual-edit"
                else -> null
            }
        }

        assertEquals(listOf(refreshedPath), readPaths)
        assertEquals(null, trackedFiles[refreshedPath])
        assertEquals(unrelatedTracked, trackedFiles[unrelatedPath])

        val entries = entriesByAgent["copilot"].orEmpty()
        assertEquals(1, entries.size)
        assertEquals("refreshed.txt", entries[0].relativePath)
        assertEquals("after-refresh", entries[0].content)
    }

    @Test
    fun sweepPreservesNewerTrackedEntryWhenUpdatedDuringProcessing() {
        val workspaceRoot = "/repo"
        val filePath = "$workspaceRoot/file.txt"

        val originalTracked = TrackedAgent(
            agentName = "copilot",
            workspaceRoot = workspaceRoot,
            lastCheckpointContent = "before",
            trackedAt = 1_000L
        )
        val updatedTracked = originalTracked.copy(lastCheckpointContent = "newer")

        val trackedFiles = ConcurrentHashMap<String, TrackedAgent>().apply {
            put(filePath, originalTracked)
        }

        val entriesByAgent = collectSweepEntriesForPaths(
            workspaceRoot = workspaceRoot,
            pathsToSweep = listOf(filePath),
            agentTouchedFiles = trackedFiles,
            now = 2_000L
        ) {
            // Simulate a concurrent in-editor update that rewrites the tracked entry.
            trackedFiles[filePath] = updatedTracked
            "disk-refresh-content"
        }

        assertTrue(entriesByAgent.isEmpty())
        assertEquals(updatedTracked, trackedFiles[filePath])
    }
}
