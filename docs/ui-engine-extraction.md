# UI Engine Extraction Plan

Extract lanterna-specific code into a reusable TUI engine with clean abstractions, enabling future backend swaps (e.g., crossterm, notcurses, or custom GPU renderer).

## Motivation

- **Testability**: Mock the graphics layer for unit testing renderers
- **Portability**: Swap lanterna for other TUI backends without touching rendering logic
- **Separation of concerns**: Clear boundary between "what to draw" and "how to draw"
- **Reusability**: Engine could be extracted to separate library for other TUI apps

## Current State

Lanterna types are used directly in 18 files:
- `TextGraphics` passed through all renderers
- `KeyStroke` used for input events
- `TerminalSize` for dimensions
- `SGR` for text styles (bold, etc.)
- `TextColor.ANSI` for colors

Already abstracted (good foundation):
- `Screen` wraps lanterna terminal
- `DrawUtils` provides `putString`, `setColor`, etc.
- `Theme` defines semantic colors
- Renderers are per-view classes

## Target Architecture

```
src/main/java/com/tuidaw/
├── engine/                          # NEW - Abstract TUI engine
│   ├── Graphics.java                # Interface for drawing operations
│   ├── InputEvent.java              # Abstract input event
│   ├── KeyCode.java                 # Enum for special keys
│   ├── Style.java                   # Text styling (bold, colors)
│   ├── Color.java                   # Abstract color (not lanterna-specific)
│   ├── ScreenBuffer.java            # Interface for screen operations
│   ├── Engine.java                  # Main loop abstraction
│   └── lanterna/                    # Lanterna implementation
│       ├── LanternaGraphics.java    # Implements Graphics
│       ├── LanternaScreen.java      # Implements ScreenBuffer
│       └── LanternaInput.java       # Converts KeyStroke → InputEvent
│
├── tui/                             # MODIFIED - Uses engine interfaces
│   ├── TUIMain.java                 # Uses Engine instead of raw lanterna
│   ├── Screen.java                  # DELETE - replaced by engine
│   ├── TerminalSizing.java          # Uses engine types
│   ├── OffsetTextGraphics.java      # Implements Graphics (decorator)
│   ├── input/
│   │   ├── InputHandler.java        # Uses InputEvent instead of KeyStroke
│   │   └── KeyBindings.java         # Uses KeyCode enum
│   └── render/
│       ├── DrawUtils.java           # Uses Graphics interface
│       ├── Renderer.java            # Uses Graphics interface
│       └── *ViewRenderer.java       # No changes needed (use DrawUtils)
```

---

## Implementation Steps

### Step 1: Create Core Engine Interfaces

**Files to create:**
- `src/main/java/com/tuidaw/engine/Graphics.java`
- `src/main/java/com/tuidaw/engine/Style.java`
- `src/main/java/com/tuidaw/engine/Color.java`

**Graphics.java:**
```java
package com.tuidaw.engine;

/**
 * Abstract interface for 2D text-based drawing operations.
 * Implementations may use lanterna, crossterm, or other backends.
 */
public interface Graphics {
    /** Put a character at position */
    void putChar(int x, int y, char c);

    /** Put a string at position */
    void putString(int x, int y, String text);

    /** Set foreground color for subsequent operations */
    void setForegroundColor(Color color);

    /** Set background color for subsequent operations */
    void setBackgroundColor(Color color);

    /** Enable/disable bold */
    void setBold(boolean bold);

    /** Enable/disable underline */
    void setUnderline(boolean underline);

    /** Reset all styles to default */
    void resetStyle();

    /** Get the width of the drawable area */
    int getWidth();

    /** Get the height of the drawable area */
    int getHeight();

    /** Fill a rectangular region with a character */
    void fillRect(int x, int y, int width, int height, char c);

    /** Draw a horizontal line */
    void drawHLine(int x, int y, int length, char c);

    /** Draw a vertical line */
    void drawVLine(int x, int y, int length, char c);
}
```

**Color.java:**
```java
package com.tuidaw.engine;

/**
 * Abstract color representation.
 * Can be converted to backend-specific color by implementations.
 */
public record Color(int r, int g, int b) {
    // Standard ANSI colors as constants
    public static final Color BLACK = new Color(0, 0, 0);
    public static final Color RED = new Color(205, 49, 49);
    public static final Color GREEN = new Color(13, 188, 121);
    public static final Color YELLOW = new Color(229, 229, 16);
    public static final Color BLUE = new Color(36, 114, 200);
    public static final Color MAGENTA = new Color(188, 63, 188);
    public static final Color CYAN = new Color(17, 168, 205);
    public static final Color WHITE = new Color(229, 229, 229);
    public static final Color DEFAULT = new Color(-1, -1, -1); // Backend default

    public boolean isDefault() {
        return r == -1 && g == -1 && b == -1;
    }
}
```

**Style.java:**
```java
package com.tuidaw.engine;

/**
 * Immutable text style combining colors and attributes.
 */
public record Style(Color foreground, Color background, boolean bold, boolean underline) {
    public static final Style DEFAULT = new Style(Color.DEFAULT, Color.DEFAULT, false, false);

    public Style withForeground(Color fg) {
        return new Style(fg, background, bold, underline);
    }

    public Style withBackground(Color bg) {
        return new Style(foreground, bg, bold, underline);
    }

    public Style withBold(boolean b) {
        return new Style(foreground, background, b, underline);
    }
}
```

**Verification:** Files compile with `mvn compile`

---

### Step 2: Create Input Abstractions

**Files to create:**
- `src/main/java/com/tuidaw/engine/KeyCode.java`
- `src/main/java/com/tuidaw/engine/InputEvent.java`

**KeyCode.java:**
```java
package com.tuidaw.engine;

/**
 * Special key codes (non-character keys).
 */
public enum KeyCode {
    ENTER, ESCAPE, BACKSPACE, DELETE, TAB,
    UP, DOWN, LEFT, RIGHT,
    HOME, END, PAGE_UP, PAGE_DOWN,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    INSERT,
    CHARACTER  // Regular character key - check getChar()
}
```

**InputEvent.java:**
```java
package com.tuidaw.engine;

/**
 * Abstract input event from keyboard.
 */
public record InputEvent(
    KeyCode keyCode,
    Character character,  // Only set if keyCode == CHARACTER
    boolean ctrl,
    boolean alt,
    boolean shift
) {
    public static InputEvent character(char c, boolean ctrl, boolean alt, boolean shift) {
        return new InputEvent(KeyCode.CHARACTER, c, ctrl, alt, shift);
    }

    public static InputEvent special(KeyCode code, boolean ctrl, boolean alt, boolean shift) {
        return new InputEvent(code, null, ctrl, alt, shift);
    }

    public boolean isCharacter() {
        return keyCode == KeyCode.CHARACTER && character != null;
    }
}
```

**Verification:** Files compile with `mvn compile`

---

### Step 3: Create Screen Buffer Interface

**Files to create:**
- `src/main/java/com/tuidaw/engine/ScreenBuffer.java`
- `src/main/java/com/tuidaw/engine/Size.java`

**Size.java:**
```java
package com.tuidaw.engine;

/**
 * Screen dimensions.
 */
public record Size(int width, int height) {}
```

**ScreenBuffer.java:**
```java
package com.tuidaw.engine;

import java.io.IOException;

/**
 * Abstract screen buffer for double-buffered rendering.
 */
public interface ScreenBuffer {
    /** Start the screen (enter alternate buffer, hide cursor, etc.) */
    void start() throws IOException;

    /** Stop the screen (restore terminal state) */
    void stop() throws IOException;

    /** Clear the entire screen */
    void clear();

    /** Flush changes to the terminal */
    void refresh() throws IOException;

    /** Get a Graphics context for drawing */
    Graphics getGraphics();

    /** Get current terminal size */
    Size getSize();

    /** Poll for input (non-blocking, returns null if none) */
    InputEvent pollInput() throws IOException;

    /** Read input (blocking) */
    InputEvent readInput() throws IOException;
}
```

**Verification:** Files compile with `mvn compile`

---

### Step 4: Create Lanterna Implementations

**Files to create:**
- `src/main/java/com/tuidaw/engine/lanterna/LanternaGraphics.java`
- `src/main/java/com/tuidaw/engine/lanterna/LanternaScreen.java`
- `src/main/java/com/tuidaw/engine/lanterna/LanternaInput.java`

**LanternaGraphics.java:**
```java
package com.tuidaw.engine.lanterna;

import com.googlecode.lanterna.SGR;
import com.googlecode.lanterna.TextColor;
import com.googlecode.lanterna.graphics.TextGraphics;
import com.tuidaw.engine.Color;
import com.tuidaw.engine.Graphics;

/**
 * Lanterna implementation of Graphics interface.
 */
public class LanternaGraphics implements Graphics {
    private final TextGraphics tg;

    public LanternaGraphics(TextGraphics tg) {
        this.tg = tg;
    }

    @Override
    public void putChar(int x, int y, char c) {
        tg.setCharacter(x, y, c);
    }

    @Override
    public void putString(int x, int y, String text) {
        tg.putString(x, y, text);
    }

    @Override
    public void setForegroundColor(Color color) {
        tg.setForegroundColor(toTextColor(color));
    }

    @Override
    public void setBackgroundColor(Color color) {
        tg.setBackgroundColor(toTextColor(color));
    }

    @Override
    public void setBold(boolean bold) {
        if (bold) {
            tg.enableModifiers(SGR.BOLD);
        } else {
            tg.disableModifiers(SGR.BOLD);
        }
    }

    @Override
    public void setUnderline(boolean underline) {
        if (underline) {
            tg.enableModifiers(SGR.UNDERLINE);
        } else {
            tg.disableModifiers(SGR.UNDERLINE);
        }
    }

    @Override
    public void resetStyle() {
        tg.setForegroundColor(TextColor.ANSI.DEFAULT);
        tg.setBackgroundColor(TextColor.ANSI.DEFAULT);
        tg.disableModifiers(SGR.BOLD, SGR.UNDERLINE);
    }

    @Override
    public int getWidth() {
        return tg.getSize().getColumns();
    }

    @Override
    public int getHeight() {
        return tg.getSize().getRows();
    }

    @Override
    public void fillRect(int x, int y, int width, int height, char c) {
        for (int row = 0; row < height; row++) {
            for (int col = 0; col < width; col++) {
                tg.setCharacter(x + col, y + row, c);
            }
        }
    }

    @Override
    public void drawHLine(int x, int y, int length, char c) {
        for (int i = 0; i < length; i++) {
            tg.setCharacter(x + i, y, c);
        }
    }

    @Override
    public void drawVLine(int x, int y, int length, char c) {
        for (int i = 0; i < length; i++) {
            tg.setCharacter(x, y + i, c);
        }
    }

    private TextColor toTextColor(Color c) {
        if (c.isDefault()) return TextColor.ANSI.DEFAULT;
        // Map to nearest ANSI color or use RGB if terminal supports
        return new TextColor.RGB(c.r(), c.g(), c.b());
    }
}
```

**LanternaInput.java:**
```java
package com.tuidaw.engine.lanterna;

import com.googlecode.lanterna.input.KeyStroke;
import com.googlecode.lanterna.input.KeyType;
import com.tuidaw.engine.InputEvent;
import com.tuidaw.engine.KeyCode;

/**
 * Converts lanterna KeyStroke to engine InputEvent.
 */
public class LanternaInput {

    public static InputEvent convert(KeyStroke ks) {
        if (ks == null) return null;

        boolean ctrl = ks.isCtrlDown();
        boolean alt = ks.isAltDown();
        boolean shift = ks.isShiftDown();

        KeyType type = ks.getKeyType();

        if (type == KeyType.Character) {
            return InputEvent.character(ks.getCharacter(), ctrl, alt, shift);
        }

        KeyCode code = switch (type) {
            case Enter -> KeyCode.ENTER;
            case Escape -> KeyCode.ESCAPE;
            case Backspace -> KeyCode.BACKSPACE;
            case Delete -> KeyCode.DELETE;
            case Tab -> KeyCode.TAB;
            case ArrowUp -> KeyCode.UP;
            case ArrowDown -> KeyCode.DOWN;
            case ArrowLeft -> KeyCode.LEFT;
            case ArrowRight -> KeyCode.RIGHT;
            case Home -> KeyCode.HOME;
            case End -> KeyCode.END;
            case PageUp -> KeyCode.PAGE_UP;
            case PageDown -> KeyCode.PAGE_DOWN;
            case Insert -> KeyCode.INSERT;
            case F1 -> KeyCode.F1;
            case F2 -> KeyCode.F2;
            case F3 -> KeyCode.F3;
            case F4 -> KeyCode.F4;
            case F5 -> KeyCode.F5;
            case F6 -> KeyCode.F6;
            case F7 -> KeyCode.F7;
            case F8 -> KeyCode.F8;
            case F9 -> KeyCode.F9;
            case F10 -> KeyCode.F10;
            case F11 -> KeyCode.F11;
            case F12 -> KeyCode.F12;
            default -> null;
        };

        if (code == null) return null;
        return InputEvent.special(code, ctrl, alt, shift);
    }
}
```

**LanternaScreen.java:**
```java
package com.tuidaw.engine.lanterna;

import com.googlecode.lanterna.TerminalSize;
import com.googlecode.lanterna.screen.TerminalScreen;
import com.googlecode.lanterna.terminal.DefaultTerminalFactory;
import com.googlecode.lanterna.terminal.Terminal;
import com.tuidaw.engine.*;

import java.io.IOException;

/**
 * Lanterna implementation of ScreenBuffer.
 */
public class LanternaScreen implements ScreenBuffer {
    private final Terminal terminal;
    private final TerminalScreen screen;

    public LanternaScreen() throws IOException {
        DefaultTerminalFactory factory = new DefaultTerminalFactory();
        this.terminal = factory.createTerminal();
        this.screen = new TerminalScreen(terminal);
    }

    @Override
    public void start() throws IOException {
        screen.startScreen();
        screen.setCursorPosition(null);
    }

    @Override
    public void stop() throws IOException {
        screen.stopScreen();
        terminal.close();
    }

    @Override
    public void clear() {
        screen.clear();
    }

    @Override
    public void refresh() throws IOException {
        screen.refresh(com.googlecode.lanterna.screen.Screen.RefreshType.DELTA);
    }

    @Override
    public Graphics getGraphics() {
        return new LanternaGraphics(screen.newTextGraphics());
    }

    @Override
    public Size getSize() {
        TerminalSize ts = screen.getTerminalSize();
        return new Size(ts.getColumns(), ts.getRows());
    }

    @Override
    public InputEvent pollInput() throws IOException {
        return LanternaInput.convert(screen.pollInput());
    }

    @Override
    public InputEvent readInput() throws IOException {
        return LanternaInput.convert(screen.readInput());
    }
}
```

**Verification:**
- Files compile with `mvn compile`
- Manually test: create a simple main() that uses LanternaScreen to draw "Hello"

---

### Step 5: Update DrawUtils to Use Engine Types

**File to modify:** `src/main/java/com/tuidaw/tui/render/DrawUtils.java`

**Changes:**
1. Replace `TextGraphics` parameter with `Graphics` from engine
2. Keep SemanticColor but have it return `engine.Color` instead of `TextColor.ANSI`
3. Update all helper methods to use `Graphics` interface

**Key changes:**
```java
// Before
public static void putString(TextGraphics g, int x, int y, String text) {
    g.putString(x, y, text);
}

// After
public static void putString(Graphics g, int x, int y, String text) {
    g.putString(x, y, text);
}

// Before
public static void setSemanticColor(TextGraphics g, SemanticColor color) {
    g.setForegroundColor(color.getAnsi());
}

// After
public static void setSemanticColor(Graphics g, SemanticColor color) {
    g.setForegroundColor(color.getColor());
}
```

**Also update Theme.java** to return `engine.Color` instead of `TextColor.ANSI`.

**Verification:** `mvn compile` succeeds (will fail until renderers updated)

---

### Step 6: Update All View Renderers

**Files to modify:**
- `RackViewRenderer.java`
- `MixerViewRenderer.java`
- `SequencerViewRenderer.java`
- `EditViewRenderer.java`
- `AddViewRenderer.java`
- `PatchViewRenderer.java`
- `HelpViewRenderer.java`
- `SeqTargetViewRenderer.java`
- `StatusBarRenderer.java`
- `Renderer.java`

**Changes per file:**
1. Change import from `com.googlecode.lanterna.graphics.TextGraphics` to `com.tuidaw.engine.Graphics`
2. Update method signatures to use `Graphics` instead of `TextGraphics`
3. No other changes needed - DrawUtils handles the abstraction

**Example diff:**
```java
// Before
import com.googlecode.lanterna.graphics.TextGraphics;

public class RackViewRenderer {
    public void render(TextGraphics g, RackState state) {

// After
import com.tuidaw.engine.Graphics;

public class RackViewRenderer {
    public void render(Graphics g, RackState state) {
```

**Verification:** `mvn compile` succeeds

---

### Step 7: Update Input Handling

**Files to modify:**
- `src/main/java/com/tuidaw/tui/input/KeyBindings.java`
- `src/main/java/com/tuidaw/tui/input/InputHandler.java`

**KeyBindings.java changes:**
```java
// Before
public static Action translate(KeyStroke key) {
    KeyType keyType = key.getKeyType();
    Character ch = keyType == KeyType.Character ? key.getCharacter() : null;

// After
public static Action translate(InputEvent event) {
    if (event.isCharacter()) {
        Character ch = event.character();
        boolean ctrl = event.ctrl();
        // ... rest of logic
    }

    return switch (event.keyCode()) {
        case UP -> event.ctrl() ? Action.MOVE_MODULE_UP : Action.MOVE_UP;
        case DOWN -> event.ctrl() ? Action.MOVE_MODULE_DOWN : Action.MOVE_DOWN;
        // ... etc
    };
}
```

**InputHandler.java changes:**
- Update to use `InputEvent` instead of `KeyStroke`
- Use `KeyCode` enum instead of `KeyType`

**Verification:** `mvn compile` succeeds

---

### Step 8: Update TUIMain and Delete Old Screen

**Files to modify:**
- `src/main/java/com/tuidaw/tui/TUIMain.java`

**File to delete:**
- `src/main/java/com/tuidaw/tui/Screen.java`

**TUIMain.java changes:**
```java
// Before
public class TUIMain {
    private final Screen screen;

    public TUIMain(Dispatcher dispatcher) throws IOException {
        this.screen = new Screen();

// After
public class TUIMain {
    private final ScreenBuffer screen;

    public TUIMain(Dispatcher dispatcher) throws IOException {
        this.screen = new LanternaScreen();
        this.screen.start();
```

**Update the run loop:**
```java
// Before
KeyStroke key = screen.pollInput();
if (key != null) {
    handleInput(key);
}

// After
InputEvent event = screen.pollInput();
if (event != null) {
    handleInput(event);
}
```

**Verification:**
- `mvn compile` succeeds
- `mvn test` passes
- Manual test: run the app and verify it works

---

### Step 9: Update TerminalSizing and OffsetTextGraphics

**Files to modify:**
- `src/main/java/com/tuidaw/tui/TerminalSizing.java`
- `src/main/java/com/tuidaw/tui/OffsetTextGraphics.java`

**TerminalSizing.java:**
- Change `TerminalSize` to `Size` from engine
- Update constructor and methods

**OffsetTextGraphics.java:**
- Rename to `OffsetGraphics.java`
- Implement `Graphics` interface instead of extending lanterna class
- Wrap a `Graphics` instance and add offsets

```java
public class OffsetGraphics implements Graphics {
    private final Graphics delegate;
    private final int offsetX;
    private final int offsetY;

    @Override
    public void putString(int x, int y, String text) {
        delegate.putString(x + offsetX, y + offsetY, text);
    }
    // ... delegate all other methods with offset
}
```

**Verification:** `mvn test` passes

---

### Step 10: Update Tests and Final Cleanup

**Files to check/update:**
- Any test files that mock or use lanterna types directly
- E2E tests should still work (they use tmux, not lanterna directly)

**Final verification:**
1. `mvn clean compile` - no lanterna imports outside `engine/lanterna/`
2. `mvn test` - all tests pass
3. Manual test - app runs correctly
4. Run: `grep -r "com.googlecode.lanterna" src/main/java --include="*.java" | grep -v engine/lanterna` - should return nothing

---

## Future Possibilities

Once extraction is complete:

1. **Mock backend for testing**: Create `TestGraphics` that records draw calls for assertion
2. **Alternative backends**: crossterm-java, JNI to notcurses, etc.
3. **Separate library**: Extract `com.tuidaw.engine` to its own Maven artifact
4. **GPU acceleration**: Backend that renders to OpenGL texture for embedding in game engines

## Estimated Effort

| Step | Files | Effort |
|------|-------|--------|
| 1. Core interfaces | 3 new | 20 min |
| 2. Input abstractions | 2 new | 15 min |
| 3. Screen buffer | 2 new | 10 min |
| 4. Lanterna impl | 3 new | 30 min |
| 5. Update DrawUtils | 2 mod | 20 min |
| 6. Update renderers | 10 mod | 30 min |
| 7. Update input | 2 mod | 20 min |
| 8. Update TUIMain | 2 mod | 15 min |
| 9. Update sizing | 2 mod | 15 min |
| 10. Tests & cleanup | varies | 20 min |

**Total: ~3 hours**
