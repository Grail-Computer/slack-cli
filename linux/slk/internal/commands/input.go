package commands

import (
	"fmt"
	"io"
	"os"
	"strings"
)

const agentFooter = "_Sent by Agent_"

// withAgentFooter marks content posted by an agent so it is distinguishable
// from messages typed manually. It is idempotent for generic API callers.
func withAgentFooter(text string) string {
	trimmed := strings.TrimRight(text, " \t\r\n")
	if strings.HasSuffix(trimmed, agentFooter) {
		return text
	}
	return trimmed + "\n\n" + agentFooter
}

// readContent resolves the user-supplied content from either an inline string
// flag (e.g. --markdown) or a file path flag (e.g. --markdown-file). Exactly
// one must be set. A filePath of "-" reads from stdin.
//
// inlineFlag and fileFlag are the human-readable flag names (with leading "--")
// used only in error messages.
func readContent(inline, filePath, inlineFlag, fileFlag string) (string, error) {
	if inline != "" && filePath != "" {
		return "", fmt.Errorf("specify %s or %s, not both", inlineFlag, fileFlag)
	}
	if inline == "" && filePath == "" {
		return "", fmt.Errorf("%s or %s is required", inlineFlag, fileFlag)
	}
	if filePath != "" {
		if filePath == "-" {
			data, err := io.ReadAll(os.Stdin)
			if err != nil {
				return "", fmt.Errorf("reading stdin: %w", err)
			}
			return string(data), nil
		}
		data, err := os.ReadFile(filePath)
		if err != nil {
			return "", err
		}
		return string(data), nil
	}
	return inline, nil
}
