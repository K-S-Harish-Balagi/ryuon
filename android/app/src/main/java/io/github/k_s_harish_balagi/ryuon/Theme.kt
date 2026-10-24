// SPDX-FileCopyrightText: 2026  Harish
// SPDX-License-Identifier: GPL-3.0-or-later

package io.github.k_s_harish_balagi.ryuon

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** The desktop app's colours (GNOME's libadwaita), so Ryuon looks like Ryuon on every system */
object Adwaita {
    val Accent = Color(0xFF3584E4)
    val DarkWindow = Color(0xFF242424)
    val DarkCard = Color(0xFF383838)
    val LightWindow = Color(0xFFFAFAFA)
    val LightCard = Color(0xFFFFFFFF)
}

@Composable
fun RyuonTheme(content: @Composable () -> Unit) {
    val colors = if (isSystemInDarkTheme()) {
        darkColorScheme(
            primary = Adwaita.Accent,
            onPrimary = Color.White,
            background = Adwaita.DarkWindow,
            onBackground = Color.White,
            surface = Adwaita.DarkCard,
            onSurface = Color.White,
        )
    } else {
        lightColorScheme(
            primary = Adwaita.Accent,
            onPrimary = Color.White,
            background = Adwaita.LightWindow,
            onBackground = Color(0xFF1E1E1E),
            surface = Adwaita.LightCard,
            onSurface = Color(0xFF1E1E1E),
        )
    }
    MaterialTheme(colorScheme = colors, content = content)
}
