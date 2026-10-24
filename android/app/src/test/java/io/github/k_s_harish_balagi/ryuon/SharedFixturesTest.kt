// SPDX-FileCopyrightText: 2026  Harish
// SPDX-License-Identifier: GPL-3.0-or-later

package io.github.k_s_harish_balagi.ryuon

import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The fixtures the desktop app's Rust tests also check (tests/fixtures/ryuon-fixtures.json):
 * real folder and song names with the desktop's answers. In A0 this only checks they load;
 * the Kotlin Tidy Names and folder order (A1) are tested against every case in them.
 */
class SharedFixturesTest {
    private val fixtures: JSONObject by lazy {
        val path = System.getProperty("ryuon.fixtures") ?: error("ryuon.fixtures not set")
        JSONObject(File(path).readText())
    }

    @Test
    fun fixturesLoadWithEveryKind() {
        assertEquals(1, fixtures.getInt("version"))
        for (key in listOf("name", "title", "artist")) {
            val pairs = fixtures.getJSONArray(key)
            assertTrue("$key has cases", pairs.length() > 100)
            for (i in 0 until pairs.length()) {
                assertEquals("$key[$i] is [input, answer]", 2, pairs.getJSONArray(i).length())
            }
        }
    }

    @Test
    fun folderOrderHasNoDuplicates() {
        val order = fixtures.getJSONArray("folder_order")
        val names = (0 until order.length()).map { order.getString(it) }
        assertTrue(names.size > 100)
        assertEquals(names.size, names.toSet().size)
    }
}
