package ai.storyteller.wordcraft

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.HapticFeedbackConstants
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.magnifier
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.FormatAlignCenter
import androidx.compose.material.icons.filled.FormatAlignJustify
import androidx.compose.material.icons.filled.FormatAlignLeft
import androidx.compose.material.icons.filled.FormatAlignRight
import androidx.compose.material.icons.filled.FormatBold
import androidx.compose.material.icons.filled.FormatItalic
import androidx.compose.material.icons.filled.FormatListBulleted
import androidx.compose.material.icons.filled.FormatListNumbered
import androidx.compose.material.icons.filled.FormatStrikethrough
import androidx.compose.material.icons.filled.FormatUnderlined
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Redo
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.TableChart
import androidx.compose.material.icons.filled.TextFields
import androidx.compose.material.icons.filled.TrackChanges
import androidx.compose.material.icons.filled.Undo
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.app.ShareCompat
import androidx.core.content.FileProvider
import kotlinx.coroutines.launch
import java.io.File
import java.io.FileOutputStream
import kotlin.math.roundToInt

class MainActivity : ComponentActivity() {

    private lateinit var bridge: WordcraftBridge

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Initialize Rust session with sample document
        bridge = WordcraftBridge(sample = true)

        setContent {
            MaterialTheme {
                WordcraftApp(bridge = bridge, onShare = ::shareDocument)
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        bridge.destroy()
    }

    private fun shareDocument(mimeType: String, bytes: ByteArray, defaultName: String) {
        try {
            val cacheFile = File(cacheDir, defaultName)
            FileOutputStream(cacheFile).use { it.write(bytes) }
            val uri: Uri = FileProvider.getUriForFile(
                this,
                "${applicationContext.packageName}.fileprovider",
                cacheFile
            )
            val shareIntent = ShareCompat.IntentBuilder(this)
                .setType(mimeType)
                .setStream(uri)
                .setChooserTitle("Share Document")
                .createChooserIntent()
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            startActivity(shareIntent)
        } catch (e: Exception) {
            Toast.makeText(this, "Failed to share: ${e.message}", Toast.LENGTH_SHORT).show()
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun WordcraftApp(
    bridge: WordcraftBridge,
    onShare: (mimeType: String, bytes: ByteArray, defaultName: String) -> Unit
) {
    val context = LocalContext.current
    val view = LocalView.current
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()

    val uiState by bridge.uiState.collectAsState()
    val isInteracting by bridge.isInteracting.collectAsState()

    var showMenu by remember { mutableStateOf(false) }
    var showStylesSheet by remember { mutableStateOf(false) }
    var showTablesSheet by remember { mutableStateOf(false) }
    var showReviewSheet by remember { mutableStateOf(false) }

    var draggingHandleRole by remember { mutableStateOf<Int?>(null) } // 0 = start, 1 = end
    var magnifierOffset by remember { mutableStateOf(Offset.Zero) }

    // WindowInsets tracking
    val imeInsets = WindowInsets.ime
    val systemBarInsets = WindowInsets.systemBars

    LaunchedEffect(imeInsets, systemBarInsets) {
        val statusBarPx = systemBarInsets.getTop(density).toFloat()
        val navBarPx = systemBarInsets.getBottom(density).toFloat()
        val imeHeightPx = imeInsets.getBottom(density).toFloat()
        val screenWidthPx = view.width.toFloat()
        val screenHeightPx = view.height.toFloat()

        bridge.updateInsets(
            statusBar = statusBarPx,
            navBar = navBarPx,
            imeHeight = imeHeightPx,
            screenWidth = screenWidthPx,
            screenHeight = screenHeightPx
        )
    }

    // SAF Open Document Launcher
    val openLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri: Uri? ->
        uri?.let {
            scope.launch {
                try {
                    context.contentResolver.openInputStream(it)?.use { stream ->
                        val bytes = stream.readBytes()
                        val filename = it.lastPathSegment ?: "document.docx"
                        bridge.openDocument(filename, bytes)
                    }
                } catch (e: Exception) {
                    Toast.makeText(context, "Failed to open file: ${e.message}", Toast.LENGTH_SHORT).show()
                }
            }
        }
    }

    // SAF Save Document Launcher
    val saveLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.CreateDocument("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
    ) { uri: Uri? ->
        uri?.let {
            scope.launch {
                try {
                    val bytes = bridge.saveDocument("document.docx")
                    if (bytes != null) {
                        context.contentResolver.openOutputStream(it)?.use { stream ->
                            stream.write(bytes)
                        }
                        Toast.makeText(context, "Saved successfully", Toast.LENGTH_SHORT).show()
                    }
                } catch (e: Exception) {
                    Toast.makeText(context, "Failed to save file: ${e.message}", Toast.LENGTH_SHORT).show()
                }
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(
                            text = "WordCraft",
                            style = MaterialTheme.typography.titleMedium
                        )
                        Text(
                            text = "${uiState.pageCount} Pages • ${uiState.wordCount} Words",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                },
                navigationIcon = {
                    IconButton(onClick = { /* Drawer toggle */ }) {
                        Icon(Icons.Default.Menu, contentDescription = "Menu")
                    }
                },
                actions = {
                    IconButton(
                        onClick = { bridge.dispatchCommand("edit.undo") },
                        enabled = uiState.canUndo
                    ) {
                        Icon(Icons.Default.Undo, contentDescription = "Undo")
                    }
                    IconButton(
                        onClick = { bridge.dispatchCommand("edit.redo") },
                        enabled = uiState.canRedo
                    ) {
                        Icon(Icons.Default.Redo, contentDescription = "Redo")
                    }
                    IconButton(
                        onClick = {
                            val pdfBytes = bridge.exportPdf()
                            if (pdfBytes != null) {
                                onShare("application/pdf", pdfBytes, "WordCraft_Export.pdf")
                            }
                        }
                    ) {
                        Icon(Icons.Default.Share, contentDescription = "Share")
                    }
                    IconButton(onClick = { showMenu = true }) {
                        Icon(Icons.Default.MoreVert, contentDescription = "More")
                    }
                    DropdownMenu(
                        expanded = showMenu,
                        onDismissRequest = { showMenu = false }
                    ) {
                        DropdownMenuItem(
                            text = { Text("Open Document…") },
                            onClick = {
                                showMenu = false
                                openLauncher.launch(
                                    arrayOf(
                                        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                                        "application/vnd.oasis.opendocument.text",
                                        "application/rtf",
                                        "text/plain",
                                        "text/markdown"
                                    )
                                )
                            }
                        )
                        DropdownMenuItem(
                            text = { Text("Save Document…") },
                            onClick = {
                                showMenu = false
                                saveLauncher.launch("document.docx")
                            }
                        )
                        DropdownMenuItem(
                            text = { Text("Export PDF") },
                            onClick = {
                                showMenu = false
                                val pdfBytes = bridge.exportPdf()
                                if (pdfBytes != null) {
                                    onShare("application/pdf", pdfBytes, "document.pdf")
                                }
                            }
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer
                )
            )
        },
        bottomBar = {
            KeyboardFormattingBar(
                uiState = uiState,
                onCommand = { id -> bridge.dispatchCommand(id) },
                onOpenStyles = { showStylesSheet = true },
                onOpenTables = { showTablesSheet = true },
                onOpenReview = { showReviewSheet = true }
            )
        }
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
        ) {
            // Document Canvas (SurfaceView)
            AndroidView(
                factory = { ctx -> WordcraftCanvas(ctx, bridge) },
                modifier = Modifier.fillMaxSize()
            )

            // Draggable Teardrop Selection Handles (visible when selection active and not interacting)
            if (uiState.hasSelection && !isInteracting) {
                // Start Handle
                TeardropHandle(
                    x = uiState.selectionStartX,
                    y = uiState.selectionStartY,
                    isStart = true,
                    onDrag = { dx, dy, screenX, screenY ->
                        draggingHandleRole = 0
                        magnifierOffset = Offset(screenX, screenY)
                        val res = bridge.onDragSelectionHandle(0, screenX, screenY)
                        if (res == 2) {
                            draggingHandleRole = 1
                            view.performHapticFeedback(HapticFeedbackConstants.TEXT_HANDLE_MOVE)
                        } else if (res == 1) {
                            view.performHapticFeedback(HapticFeedbackConstants.TEXT_HANDLE_MOVE)
                        }
                    },
                    onDragEnd = { draggingHandleRole = null }
                )

                // End Handle
                TeardropHandle(
                    x = uiState.selectionEndX,
                    y = uiState.selectionEndY,
                    isStart = false,
                    onDrag = { dx, dy, screenX, screenY ->
                        draggingHandleRole = 1
                        magnifierOffset = Offset(screenX, screenY)
                        val res = bridge.onDragSelectionHandle(1, screenX, screenY)
                        if (res == 2) {
                            draggingHandleRole = 0
                            view.performHapticFeedback(HapticFeedbackConstants.TEXT_HANDLE_MOVE)
                        } else if (res == 1) {
                            view.performHapticFeedback(HapticFeedbackConstants.TEXT_HANDLE_MOVE)
                        }
                    },
                    onDragEnd = { draggingHandleRole = null }
                )
            }

            // Magnifier Lens (Active when dragging a handle)
            if (draggingHandleRole != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                Box(
                    modifier = Modifier
                        .offset { IntOffset(magnifierOffset.x.roundToInt(), magnifierOffset.y.roundToInt()) }
                        .size(1.dp)
                        .magnifier(
                            sourceCenter = { magnifierOffset }
                        )
                )
            }

            // Floating Selection Toolbar (Anchored above selection bounds)
            AnimatedVisibility(
                visible = uiState.hasSelection && !isInteracting && draggingHandleRole == null,
                enter = fadeIn(),
                exit = fadeOut(),
                modifier = Modifier.align(Alignment.TopStart)
            ) {
                val tbX = with(density) { uiState.selectionBoundsMinX.toDp() }
                val tbY = with(density) { (uiState.selectionBoundsMinY - 48f).coerceAtLeast(8f).toDp() }

                Card(
                    modifier = Modifier
                        .offset(x = tbX, y = tbY)
                        .shadow(8.dp, RoundedCornerShape(24.dp)),
                    shape = RoundedCornerShape(24.dp),
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh
                    )
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 4.dp, vertical = 2.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        TextButton(onClick = { bridge.dispatchCommand("edit.cut") }) {
                            Text("Cut", fontSize = 13.sp)
                        }
                        TextButton(onClick = { bridge.dispatchCommand("edit.copy") }) {
                            Text("Copy", fontSize = 13.sp)
                        }
                        TextButton(onClick = { bridge.dispatchCommand("edit.paste") }) {
                            Text("Paste", fontSize = 13.sp)
                        }
                        TextButton(onClick = { bridge.dispatchCommand("format.highlight") }) {
                            Text("Highlight", fontSize = 13.sp)
                        }
                        TextButton(onClick = { bridge.dispatchCommand("comment.new") }) {
                            Text("Comment", fontSize = 13.sp)
                        }
                    }
                }
            }
        }
    }

    // Modal Bottom Sheets for Styles, Tables, Review
    if (showStylesSheet) {
        ModalBottomSheet(
            onDismissRequest = { showStylesSheet = false },
            sheetState = rememberModalBottomSheetState()
        ) {
            StylesBottomSheet(
                currentStyle = uiState.styleName,
                onSelectStyle = { styleId ->
                    bridge.dispatchCommand("style.apply", mapOf("name" to styleId))
                    showStylesSheet = false
                }
            )
        }
    }

    if (showTablesSheet) {
        ModalBottomSheet(
            onDismissRequest = { showTablesSheet = false },
            sheetState = rememberModalBottomSheetState()
        ) {
            TablesBottomSheet(
                onInsertTable = { rows, cols ->
                    bridge.dispatchCommand("table.insert", mapOf("rows" to rows, "cols" to cols))
                    showTablesSheet = false
                }
            )
        }
    }

    if (showReviewSheet) {
        ModalBottomSheet(
            onDismissRequest = { showReviewSheet = false },
            sheetState = rememberModalBottomSheetState()
        ) {
            ReviewBottomSheet(
                onCommand = { cmd ->
                    bridge.dispatchCommand(cmd)
                    showReviewSheet = false
                }
            )
        }
    }
}

/**
 * Teardrop Selection Handle matching native Android Material 3 touch ergonomics.
 */
@Composable
fun TeardropHandle(
    x: Float,
    y: Float,
    isStart: Boolean = true,
    onDrag: (dx: Float, dy: Float, screenX: Float, screenY: Float) -> Unit,
    onDragEnd: () -> Unit
) {
    val density = LocalDensity.current
    val primaryColor = MaterialTheme.colorScheme.primary

    var currentX by remember(x) { mutableStateOf(x) }
    var currentY by remember(y) { mutableStateOf(y) }

    val handleOffset = with(density) {
        IntOffset((currentX - 16f).roundToInt(), currentY.roundToInt())
    }

    Box(
        modifier = Modifier
            .offset { handleOffset }
            .size(32.dp)
            .pointerInput(Unit) {
                detectDragGestures(
                    onDragEnd = onDragEnd,
                    onDragCancel = onDragEnd,
                    onDrag = { change, dragAmount ->
                        change.consume()
                        currentX += dragAmount.x
                        currentY += dragAmount.y
                        onDrag(dragAmount.x, dragAmount.y, currentX, currentY)
                    }
                )
            }
    ) {
        Canvas(modifier = Modifier.fillMaxSize()) {
            val radius = size.width * 0.45f
            val bulbCenter = Offset(size.width * 0.5f, size.height * 0.5f)

            val path = Path().apply {
                if (isStart) {
                    moveTo(size.width * 0.5f, 0f)
                    lineTo(size.width, size.height * 0.5f)
                    arcTo(
                        rect = androidx.compose.ui.geometry.Rect(
                            bulbCenter.x - radius, bulbCenter.y - radius,
                            bulbCenter.x + radius, bulbCenter.y + radius
                        ),
                        startAngleDegrees = 0f,
                        sweepAngleDegrees = 180f,
                        forceMoveTo = false
                    )
                    close()
                } else {
                    moveTo(size.width * 0.5f, 0f)
                    lineTo(0f, size.height * 0.5f)
                    arcTo(
                        rect = androidx.compose.ui.geometry.Rect(
                            bulbCenter.x - radius, bulbCenter.y - radius,
                            bulbCenter.x + radius, bulbCenter.y + radius
                        ),
                        startAngleDegrees = 180f,
                        sweepAngleDegrees = 180f,
                        forceMoveTo = false
                    )
                    close()
                }
            }
            drawPath(path, color = primaryColor)
        }
    }
}

/**
 * Contextual Keyboard Formatting Bar docked directly above the on-screen soft keyboard.
 */
@Composable
fun KeyboardFormattingBar(
    uiState: EditorUiState,
    onCommand: (String) -> Unit,
    onOpenStyles: () -> Unit,
    onOpenTables: () -> Unit,
    onOpenReview: () -> Unit
) {
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .imePadding(),
        color = MaterialTheme.colorScheme.surfaceContainer,
        tonalElevation = 3.dp
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 4.dp, vertical = 2.dp),
            horizontalArrangement = Arrangement.SpaceAround,
            verticalAlignment = Alignment.CenterVertically
        ) {
            IconButton(onClick = { onCommand("format.bold") }) {
                Icon(
                    Icons.Default.FormatBold,
                    contentDescription = "Bold",
                    tint = if (uiState.isBold) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface
                )
            }
            IconButton(onClick = { onCommand("format.italic") }) {
                Icon(
                    Icons.Default.FormatItalic,
                    contentDescription = "Italic",
                    tint = if (uiState.isItalic) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface
                )
            }
            IconButton(onClick = { onCommand("format.underline") }) {
                Icon(
                    Icons.Default.FormatUnderlined,
                    contentDescription = "Underline",
                    tint = if (uiState.isUnderline) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface
                )
            }
            IconButton(onClick = { onCommand("format.strikethrough") }) {
                Icon(
                    Icons.Default.FormatStrikethrough,
                    contentDescription = "Strikethrough",
                    tint = if (uiState.isStrikethrough) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface
                )
            }

            // Paragraph Alignment Toggle
            IconButton(onClick = {
                val nextAlign = when (uiState.alignment) {
                    0 -> "para.alignCenter"
                    1 -> "para.alignRight"
                    2 -> "para.alignJustify"
                    else -> "para.alignLeft"
                }
                onCommand(nextAlign)
            }) {
                val icon = when (uiState.alignment) {
                    1 -> Icons.Default.FormatAlignCenter
                    2 -> Icons.Default.FormatAlignRight
                    3 -> Icons.Default.FormatAlignJustify
                    else -> Icons.Default.FormatAlignLeft
                }
                Icon(icon, contentDescription = "Alignment")
            }

            // Lists
            IconButton(onClick = { onCommand("list.bullet") }) {
                Icon(Icons.Default.FormatListBulleted, contentDescription = "Bullet List")
            }
            IconButton(onClick = { onCommand("list.numbered") }) {
                Icon(Icons.Default.FormatListNumbered, contentDescription = "Numbered List")
            }

            // Bottom Sheet Triggers
            IconButton(onClick = onOpenStyles) {
                Icon(Icons.Default.TextFields, contentDescription = "Styles")
            }
            IconButton(onClick = onOpenTables) {
                Icon(Icons.Default.TableChart, contentDescription = "Tables")
            }
            IconButton(onClick = onOpenReview) {
                Icon(Icons.Default.TrackChanges, contentDescription = "Review")
            }
        }
    }
}

@Composable
fun StylesBottomSheet(
    currentStyle: String,
    onSelectStyle: (String) -> Unit
) {
    val styles = listOf(
        "Normal" to "Normal",
        "Heading 1" to "Heading 1",
        "Heading 2" to "Heading 2",
        "Heading 3" to "Heading 3",
        "Title" to "Title",
        "Subtitle" to "Subtitle",
        "Quote" to "Quote"
    )
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp)
    ) {
        Text("Paragraph Style", style = MaterialTheme.typography.titleLarge)
        Spacer(modifier = Modifier.height(12.dp))
        styles.forEach { (name, id) ->
            TextButton(
                onClick = { onSelectStyle(id) },
                modifier = Modifier.fillMaxWidth()
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(name, style = MaterialTheme.typography.bodyLarge)
                    if (currentStyle == id) {
                        Icon(Icons.Default.Check, contentDescription = "Selected")
                    }
                }
            }
        }
    }
}

@Composable
fun TablesBottomSheet(
    onInsertTable: (rows: Int, cols: Int) -> Unit
) {
    var rows by remember { mutableStateOf(3) }
    var cols by remember { mutableStateOf(3) }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp)
    ) {
        Text("Insert Table", style = MaterialTheme.typography.titleLarge)
        Spacer(modifier = Modifier.height(16.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("Rows: $rows")
            Row {
                TextButton(onClick = { if (rows > 1) rows-- }) { Text("-") }
                TextButton(onClick = { if (rows < 20) rows++ }) { Text("+") }
            }
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("Columns: $cols")
            Row {
                TextButton(onClick = { if (cols > 1) cols-- }) { Text("-") }
                TextButton(onClick = { if (cols < 10) cols++ }) { Text("+") }
            }
        }

        Spacer(modifier = Modifier.height(16.dp))
        TextButton(
            onClick = { onInsertTable(rows, cols) },
            modifier = Modifier.fillMaxWidth()
        ) {
            Text("Insert ${rows}x${cols} Table")
        }
    }
}

@Composable
fun ReviewBottomSheet(
    onCommand: (String) -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp)
    ) {
        Text("Track Changes & Review", style = MaterialTheme.typography.titleLarge)
        Spacer(modifier = Modifier.height(12.dp))

        TextButton(
            onClick = { onCommand("review.trackChanges") },
            modifier = Modifier.fillMaxWidth()
        ) {
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Start) {
                Icon(Icons.Default.TrackChanges, contentDescription = null)
                Spacer(modifier = Modifier.width(12.dp))
                Text("Toggle Track Changes")
            }
        }

        TextButton(
            onClick = { onCommand("review.acceptAll") },
            modifier = Modifier.fillMaxWidth()
        ) {
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Start) {
                Icon(Icons.Default.Check, contentDescription = null)
                Spacer(modifier = Modifier.width(12.dp))
                Text("Accept All Changes")
            }
        }

        TextButton(
            onClick = { onCommand("review.rejectAll") },
            modifier = Modifier.fillMaxWidth()
        ) {
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Start) {
                Icon(Icons.Default.Close, contentDescription = null)
                Spacer(modifier = Modifier.width(12.dp))
                Text("Reject All Changes")
            }
        }
    }
}
