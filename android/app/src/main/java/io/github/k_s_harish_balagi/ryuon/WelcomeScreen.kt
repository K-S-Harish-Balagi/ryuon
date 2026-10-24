// SPDX-FileCopyrightText: 2026  Harish
// SPDX-License-Identifier: GPL-3.0-or-later

package io.github.k_s_harish_balagi.ryuon

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** First start, like the desktop app's welcome view (canvas board 1) */
@Composable
fun WelcomeScreen(onChooseFolder: () -> Unit = {}, onAddSongs: () -> Unit = {}) {
    val colors = MaterialTheme.colorScheme
    BoxWithConstraints(Modifier.fillMaxSize().background(colors.background).safeDrawingPadding()) {
        val wide = maxWidth > maxHeight && maxWidth >= 560.dp
        // Each layout is centred when it fits and scrolls only when it cannot (very large text)
        val page = Modifier
            .verticalScroll(rememberScrollState())
            .fillMaxWidth()
            .heightIn(min = maxHeight)
            .padding(horizontal = 36.dp, vertical = 24.dp)
        if (wide) {
            // Landscape: the logo and name on the left, the choice on the right
            Row(page, horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Logo()
                    Spacer(Modifier.height(24.dp))
                    Text("Ryuon", color = colors.onBackground, fontSize = 30.sp, fontWeight = FontWeight.ExtraBold)
                }
                Spacer(Modifier.width(36.dp))
                Column(Modifier.weight(1.3f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Choices(onChooseFolder, onAddSongs, gap = 24.dp)
                }
            }
        } else {
            Column(page, horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
                Logo()
                Spacer(Modifier.height(36.dp))
                Text("Ryuon", color = colors.onBackground, fontSize = 30.sp, fontWeight = FontWeight.ExtraBold)
                Spacer(Modifier.height(14.dp))
                Choices(onChooseFolder, onAddSongs, gap = 40.dp)
            }
        }
    }
}

/** The explanation, the two buttons and the privacy line */
@Composable
private fun Choices(onChooseFolder: () -> Unit, onAddSongs: () -> Unit, gap: androidx.compose.ui.unit.Dp) {
    val colors = MaterialTheme.colorScheme
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            stringResource(R.string.welcome_text),
            color = colors.onBackground.copy(alpha = 0.78f),
            fontSize = 16.sp,
            lineHeight = 24.sp,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(gap))
        Button(
            onClick = onChooseFolder,
            modifier = Modifier.width(220.dp).height(52.dp),
            colors = ButtonDefaults.buttonColors(containerColor = Adwaita.Accent, contentColor = Color.White),
        ) {
            Text(stringResource(R.string.choose_music_folder), fontSize = 16.sp, fontWeight = FontWeight.Bold)
        }
        Spacer(Modifier.height(14.dp))
        Button(
            onClick = onAddSongs,
            modifier = Modifier.width(220.dp).height(52.dp),
            colors = ButtonDefaults.buttonColors(
                containerColor = colors.onBackground.copy(alpha = 0.10f),
                contentColor = colors.onBackground,
            ),
        ) {
            Text(stringResource(R.string.add_songs), fontSize = 16.sp, fontWeight = FontWeight.Bold)
        }
        Spacer(Modifier.height(gap))
        Text(
            stringResource(R.string.privacy_line),
            color = colors.onBackground.copy(alpha = 0.78f),
            fontSize = 13.sp,
            lineHeight = 20.sp,
            textAlign = TextAlign.Center,
        )
    }
}

/** The app icon at 128 dp, drawn from the same vector as the launcher icon */
@Composable
private fun Logo() {
    Box(
        modifier = Modifier
            .size(128.dp)
            .clip(RoundedCornerShape(29.dp))
            .background(Brush.linearGradient(listOf(Color(0xFF06AAC5), Color(0xFF180141)))),
        contentAlignment = Alignment.Center,
    ) {
        Image(painterResource(R.drawable.ic_launcher_foreground), contentDescription = "Ryuon logo", modifier = Modifier.size(128.dp * 108 / 72))
    }
}

@Preview
@Composable
private fun WelcomePreview() = RyuonTheme { WelcomeScreen() }
