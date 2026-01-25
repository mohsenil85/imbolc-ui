# Tmux Test Harness

End-to-end UI testing using tmux to control the TUI programmatically.

## Motivation

- Test actual rendered output, not just state transitions
- Verify keybindings work in real terminal
- Catch rendering bugs that unit tests miss
- Automate user workflows for regression testing

## Architecture

```
┌─────────────────────────────────────────────────┐
│  TmuxTestHarness.java                           │
│  ┌───────────────────────────────────────────┐  │
│  │  tmux session "tuidaw-test"               │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │  java -jar tuidaw.jar               │  │  │
│  │  │  (TUI running in pseudo-terminal)   │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────┘  │
│                                                 │
│  sendKeys("a", "Down", "Enter")                 │
│  captureScreen() → String                       │
│  assertScreenContains("saw-1")                  │
└─────────────────────────────────────────────────┘
```

## Implementation

### TmuxTestHarness.java

```java
package com.tuidaw.test.e2e;

import java.io.*;
import java.util.concurrent.TimeUnit;

public class TmuxTestHarness implements AutoCloseable {
    private static final int WIDTH = 120;
    private static final int HEIGHT = 40;

    private final String sessionName;
    private final ProcessBuilder pb;

    public TmuxTestHarness(String testName) {
        this.sessionName = "tuidaw-" + testName + "-" + System.currentTimeMillis();
        this.pb = new ProcessBuilder();
        pb.redirectErrorStream(true);
    }

    public void start() throws Exception {
        exec("tmux", "new-session", "-d", "-s", sessionName,
             "-x", String.valueOf(WIDTH), "-y", String.valueOf(HEIGHT),
             "java", "-jar", "target/tuidaw.jar");
        Thread.sleep(2000);  // Wait for app startup
    }

    public void sendKeys(String... keys) throws Exception {
        for (String key : keys) {
            exec("tmux", "send-keys", "-t", sessionName, key);
            Thread.sleep(50);  // Brief delay between keys
        }
        Thread.sleep(100);  // Wait for render
    }

    public void sendKey(String key) throws Exception {
        sendKeys(key);
    }

    public void type(String text) throws Exception {
        exec("tmux", "send-keys", "-t", sessionName, "-l", text);
        Thread.sleep(100);
    }

    public String captureScreen() throws Exception {
        return execOutput("tmux", "capture-pane", "-t", sessionName, "-p");
    }

    public void assertScreenContains(String text) throws Exception {
        String screen = captureScreen();
        if (!screen.contains(text)) {
            throw new AssertionError(
                "Expected screen to contain: " + text + "\n" +
                "Actual screen:\n" + screen
            );
        }
    }

    public void assertScreenMatches(String regex) throws Exception {
        String screen = captureScreen();
        if (!screen.matches("(?s).*" + regex + ".*")) {
            throw new AssertionError(
                "Expected screen to match: " + regex + "\n" +
                "Actual screen:\n" + screen
            );
        }
    }

    @Override
    public void close() throws Exception {
        exec("tmux", "kill-session", "-t", sessionName);
    }

    private void exec(String... cmd) throws Exception {
        pb.command(cmd);
        Process p = pb.start();
        p.waitFor(5, TimeUnit.SECONDS);
    }

    private String execOutput(String... cmd) throws Exception {
        pb.command(cmd);
        Process p = pb.start();
        try (BufferedReader r = new BufferedReader(
                new InputStreamReader(p.getInputStream()))) {
            StringBuilder sb = new StringBuilder();
            String line;
            while ((line = r.readLine()) != null) {
                sb.append(line).append("\n");
            }
            return sb.toString();
        }
    }
}
```

### Example Test

```java
package com.tuidaw.test.e2e;

import org.junit.jupiter.api.*;
import static org.assertj.core.api.Assertions.*;

@Tag("e2e")
class RackViewE2ETest {

    private TmuxTestHarness harness;

    @BeforeEach
    void setup() throws Exception {
        harness = new TmuxTestHarness("rack");
        harness.start();
    }

    @AfterEach
    void teardown() throws Exception {
        harness.close();
    }

    @Test
    void addModule_showsInRack() throws Exception {
        // Open add view
        harness.sendKey("a");
        harness.assertScreenContains("Add Module");

        // Select SAW_OSC (first option)
        harness.sendKey("Enter");

        // Verify module added
        harness.assertScreenContains("saw-1");
    }

    @Test
    void navigation_movesSelection() throws Exception {
        // Add two modules
        harness.sendKeys("a", "Enter");  // saw-1
        harness.sendKeys("a", "Down", "Enter");  // some other module

        // Navigate up
        harness.sendKey("Up");

        // Capture and verify selection indicator
        String screen = harness.captureScreen();
        // Selection should be on first module
        assertThat(screen).contains("> saw-1");
    }

    @Test
    void panic_silencesAll() throws Exception {
        harness.sendKeys("a", "Enter");  // Add module
        harness.sendKey(".");            // Panic
        harness.assertScreenContains("[PANIC!]");
    }

    @Test
    void mixerView_showsChannels() throws Exception {
        harness.sendKeys("a", "Enter");  // Add module (auto-assigns to mixer)
        harness.sendKey("m");            // Switch to mixer
        harness.assertScreenContains("Ch 1");
        harness.assertScreenContains("saw-1");
    }
}
```

## Key Mappings for tmux send-keys

| Action | tmux send-keys |
|--------|----------------|
| Arrow Up | `Up` |
| Arrow Down | `Down` |
| Arrow Left | `Left` |
| Arrow Right | `Right` |
| Enter | `Enter` |
| Escape | `Escape` |
| Space | `Space` |
| Tab | `Tab` |
| Ctrl+S | `C-s` |
| Ctrl+Z | `C-z` |
| F1 | `F1` |
| Letter 'a' | `a` |
| Shift+A | `A` |

## Running E2E Tests

```bash
# Run only E2E tests
mvn test -Dgroups=e2e

# Skip E2E tests (faster CI)
mvn test -DexcludedGroups=e2e

# Run with visible tmux (for debugging)
TUIDAW_E2E_VISIBLE=1 mvn test -Dgroups=e2e
```

## CI Integration

```yaml
# .github/workflows/e2e.yml
name: E2E Tests
on: [push, pull_request]
jobs:
  e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-java@v4
        with:
          java-version: '21'
      - name: Install tmux
        run: sudo apt-get install -y tmux
      - name: Build
        run: mvn package -DskipTests
      - name: Run E2E tests
        run: mvn test -Dgroups=e2e
```

## Screenshot Comparison (Future)

For pixel-perfect testing:

```java
public void saveScreenshot(String name) throws Exception {
    String screen = captureScreen();
    Path path = Path.of("target/e2e-screenshots", name + ".txt");
    Files.writeString(path, screen);
}

public void assertScreenshotMatches(String name) throws Exception {
    String expected = Files.readString(
        Path.of("src/test/resources/e2e-expected", name + ".txt"));
    String actual = captureScreen();
    assertThat(actual).isEqualTo(expected);
}
```
