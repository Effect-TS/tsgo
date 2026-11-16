package project

import "github.com/microsoft/TypeScript/tsc/internal/tspath"

// ContentMapperSelector mirrors the next Client.RegisterContentMapperSelectors argument. This commit's Client
// still registers plain extensions (RegisterContentMapperExtensions), so the type is only there to let one Client
// implementation compile against every supported compiler.
type ContentMapperSelector struct {
	Base      tspath.RootedDirectoryPath
	Pattern   string
	Extension string
}
