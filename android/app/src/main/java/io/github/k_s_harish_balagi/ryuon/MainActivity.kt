// SPDX-FileCopyrightText: 2026  Harish
// SPDX-License-Identifier: GPL-3.0-or-later

package io.github.k_s_harish_balagi.ryuon

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            RyuonTheme {
                // A1 wires the buttons to the folder picker and the library
                WelcomeScreen()
            }
        }
    }
}
