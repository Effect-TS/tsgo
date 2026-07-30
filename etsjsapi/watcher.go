package etsjsapi

import (
	"context"
	"errors"
	"fmt"
	"io"
	"strings"
	"sync"

	"github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/fswatch"
	"github.com/microsoft/typescript-go/shim/locale"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/project"
	"github.com/microsoft/typescript-go/shim/tspath"
)

type watchBackend interface {
	WatchDirectory(dir string, fn fswatch.WatchCallback, opts ...fswatch.WatchOption) (io.Closer, error)
	WatchFile(path string, fn fswatch.WatchCallback) (io.Closer, error)
}

type fswatchBackend struct{ watcher fswatch.Watcher }

func (b fswatchBackend) WatchDirectory(dir string, fn fswatch.WatchCallback, opts ...fswatch.WatchOption) (io.Closer, error) {
	return b.watcher.WatchDirectory(dir, fn, opts...)
}

func (b fswatchBackend) WatchFile(path string, fn fswatch.WatchCallback) (io.Closer, error) {
	return b.watcher.WatchFile(path, fn)
}

type pendingWatchChange struct {
	created bool
	changed bool
	deleted bool
}

var errVirtualWatchPath = errors.New("virtual paths do not require filesystem watches")

type localWatcherClient struct {
	backend watchBackend

	watchesMu sync.Mutex
	watches   map[project.WatcherID][]io.Closer
	closed    bool

	changesMu        sync.Mutex
	changes          map[string]pendingWatchChange
	overflow         bool
	invalidateAlways bool
	generation       uint64
}

func newLocalWatcherClient() *localWatcherClient {
	return &localWatcherClient{
		backend: fswatchBackend{watcher: fswatch.Default()},
		watches: make(map[project.WatcherID][]io.Closer),
		changes: make(map[string]pendingWatchChange),
	}
}

func (c *localWatcherClient) WatchFiles(_ context.Context, id project.WatcherID, watchers []*lsproto.FileSystemWatcher) error {
	c.watchesMu.Lock()
	defer c.watchesMu.Unlock()
	if c.closed {
		return errors.New("watcher client is closed")
	}
	c.closeWatchesLocked(id)

	opened := make([]io.Closer, 0, len(watchers))
	for _, watcher := range watchers {
		target, recursive, exact, err := watchTarget(watcher.GlobPattern)
		if errors.Is(err, errVirtualWatchPath) {
			continue
		}
		if err != nil {
			closeWatches(opened)
			return err
		}
		callback := c.callback(watcher.Kind)
		var watch io.Closer
		if exact {
			watch, err = c.backend.WatchFile(target, callback)
		} else {
			var options []fswatch.WatchOption
			if recursive {
				options = append(options, fswatch.WithRecursive())
			}
			options = append(options, fswatch.WithIgnore(ignoreWatchPath))
			watch, err = c.backend.WatchDirectory(target, callback, options...)
		}
		if err != nil {
			c.changesMu.Lock()
			c.invalidateAlways = true
			c.generation++
			c.changesMu.Unlock()
			closeWatches(opened)
			return err
		}
		opened = append(opened, watch)
	}
	c.watches[id] = opened
	return nil
}

func (c *localWatcherClient) UnwatchFiles(_ context.Context, id project.WatcherID) error {
	c.watchesMu.Lock()
	defer c.watchesMu.Unlock()
	c.closeWatchesLocked(id)
	return nil
}

func (c *localWatcherClient) callback(kind *lsproto.WatchKind) fswatch.WatchCallback {
	watchKind := lsproto.WatchKindCreate | lsproto.WatchKindChange | lsproto.WatchKindDelete
	if kind != nil {
		watchKind = *kind
	}
	return func(events []fswatch.Event, err error) {
		c.changesMu.Lock()
		defer c.changesMu.Unlock()
		if err != nil {
			if errors.Is(err, fswatch.ErrOverflow) {
				c.overflow = true
				c.generation++
			} else {
				c.invalidateAlways = true
				c.generation++
			}
			return
		}
		if len(events) == 0 {
			return
		}
		for _, event := range events {
			change := c.changes[event.Path]
			switch event.Kind {
			case fswatch.EventDelete:
				if watchKind&lsproto.WatchKindDelete != 0 {
					change.deleted = true
				}
			case fswatch.EventUpdate:
				// fswatch combines create and modify events, so every update must
				// invalidate existing disk content even for create-only registrations.
				change.changed = true
				if watchKind&lsproto.WatchKindCreate != 0 {
					change.created = true
				}
			}
			c.changes[event.Path] = change
		}
		c.generation++
	}
}

func (c *localWatcherClient) drain() (project.FileChangeSummary, uint64) {
	c.changesMu.Lock()
	defer c.changesMu.Unlock()
	result := project.FileChangeSummary{InvalidateAll: c.overflow || c.invalidateAlways}
	for fileName, change := range c.changes {
		uri := lsconv.FileNameToDocumentURI(fileName)
		if change.created {
			result.Created.Add(uri)
		}
		if change.changed {
			result.Changed.Add(uri)
		}
		if change.deleted {
			result.Deleted.Add(uri)
		}
		if !strings.Contains(tspath.NormalizeSlashes(fileName), "/node_modules/") {
			result.IncludesWatchChangeOutsideNodeModules = true
		}
	}
	clear(c.changes)
	c.overflow = false
	return result, c.generation
}

func (c *localWatcherClient) currentGeneration() uint64 {
	c.changesMu.Lock()
	defer c.changesMu.Unlock()
	return c.generation
}

func (c *localWatcherClient) close() {
	c.watchesMu.Lock()
	defer c.watchesMu.Unlock()
	c.closed = true
	for id := range c.watches {
		c.closeWatchesLocked(id)
	}
}

func (c *localWatcherClient) closeWatchesLocked(id project.WatcherID) {
	closeWatches(c.watches[id])
	delete(c.watches, id)
}

func closeWatches(watches []io.Closer) {
	for _, watch := range watches {
		_ = watch.Close()
	}
}

func watchTarget(pattern lsproto.PatternOrRelativePattern) (target string, recursive bool, exact bool, err error) {
	var value string
	switch {
	case pattern.Pattern != nil:
		value = *pattern.Pattern
		if tspath.IsUrl(value) {
			return "", false, false, errVirtualWatchPath
		}
	case pattern.RelativePattern != nil:
		baseURI := pattern.RelativePattern.BaseUri.URI
		if pattern.RelativePattern.BaseUri.WorkspaceFolder != nil {
			baseURI = &pattern.RelativePattern.BaseUri.WorkspaceFolder.Uri
		}
		if baseURI == nil {
			return "", false, false, errors.New("watch pattern has no base URI")
		}
		base := lsproto.DocumentUri(*baseURI).FileName()
		value = tspath.CombinePaths(base, pattern.RelativePattern.Pattern)
	default:
		return "", false, false, errors.New("watch pattern is empty")
	}

	value = tspath.NormalizePath(value)
	index := strings.IndexAny(value, "*?{[")
	if index == -1 {
		return value, false, true, nil
	}
	literal := value[:index]
	prefix := strings.TrimRight(literal, string(tspath.DirectorySeparator))
	rootLength := tspath.GetRootLength(literal)
	if len(prefix) < rootLength {
		prefix = literal[:rootLength]
	} else if !strings.HasSuffix(literal, string(tspath.DirectorySeparator)) {
		prefix = tspath.GetDirectoryPath(prefix)
	}
	if !tspath.IsRootedDiskPath(prefix) {
		return "", false, false, fmt.Errorf("watch pattern is not absolute: %s", value)
	}
	return prefix, true, false, nil
}

func ignoreWatchPath(path string) bool {
	normalized := tspath.NormalizeSlashes(path)
	return strings.HasSuffix(normalized, "/.git") ||
		strings.Contains(normalized, "/.git/") ||
		strings.Contains(normalized, "/node_modules/.") ||
		strings.Contains(normalized, "/.#")
}

func (*localWatcherClient) RefreshDiagnostics(context.Context) error { return nil }
func (*localWatcherClient) PublishDiagnostics(context.Context, *lsproto.PublishDiagnosticsParams) error {
	return nil
}
func (*localWatcherClient) RefreshInlayHints(context.Context) error                     { return nil }
func (*localWatcherClient) RefreshCodeLens(context.Context) error                       { return nil }
func (*localWatcherClient) ProgressStart(*diagnostics.Message, ...any)                  {}
func (*localWatcherClient) ProgressFinish(*diagnostics.Message, ...any)                 {}
func (*localWatcherClient) SendTelemetry(context.Context, lsproto.TelemetryEvent) error { return nil }
func (*localWatcherClient) IsActive() bool                                              { return true }
func (*localWatcherClient) SetLocale(string)                                            {}
func (*localWatcherClient) GetLocale() locale.Locale                                    { return locale.Default }

var _ project.Client = (*localWatcherClient)(nil)
