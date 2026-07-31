package etsjsapi

import (
	"context"
	"errors"
	"io"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/microsoft/typescript-go/shim/fswatch"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/project"
)

func TestLocalWatcherClientReceivesDiskChange(t *testing.T) {
	t.Parallel()

	if !fswatch.Default().Available() {
		t.Skip("platform watcher unavailable")
	}

	root := t.TempDir()
	nested := filepath.Join(root, "src")
	if err := os.Mkdir(nested, 0o755); err != nil {
		t.Fatal(err)
	}
	changedFile := filepath.Join(nested, "index.ts")
	if err := os.WriteFile(changedFile, []byte("export {}\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	client := newLocalWatcherClient()
	pattern := filepath.Join(root, "**", "*")
	id := project.WatcherID("recursive")
	if err := client.WatchFiles(context.Background(), id, []*lsproto.FileSystemWatcher{{
		GlobPattern: lsproto.PatternOrRelativePattern{Pattern: &pattern},
	}}); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(client.close)

	if err := os.WriteFile(changedFile, []byte("export const changed = true\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		summary, _ := client.drain()
		if summary.Created.Has(lsconv.FileNameToDocumentURI(changedFile)) || summary.Changed.Has(lsconv.FileNameToDocumentURI(changedFile)) {
			return
		}
		time.Sleep(20 * time.Millisecond)
	}
	t.Fatal("watcher did not report disk change")
}

func TestWatchTarget(t *testing.T) {
	t.Parallel()

	root := t.TempDir()
	exact := filepath.Join(root, "src", "index.ts")
	glob := filepath.Join(root, "src", "**", "*")

	tests := []struct {
		name      string
		pattern   string
		target    string
		recursive bool
		exact     bool
	}{
		{name: "exact", pattern: exact, target: exact, exact: true},
		{name: "recursive", pattern: glob, target: filepath.Join(root, "src"), recursive: true},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()

			target, recursive, exact, err := watchTarget(lsproto.PatternOrRelativePattern{Pattern: &test.pattern})
			if err != nil {
				t.Fatal(err)
			}
			if target != test.target || recursive != test.recursive || exact != test.exact {
				t.Fatalf("watchTarget() = (%q, %v, %v), want (%q, %v, %v)", target, recursive, exact, test.target, test.recursive, test.exact)
			}
		})
	}
}

func TestLocalWatcherClientDrain(t *testing.T) {
	t.Parallel()

	client := &localWatcherClient{
		watches: make(map[project.WatcherID][]io.Closer),
		changes: make(map[string]pendingWatchChange),
	}
	changedFile := filepath.Join(t.TempDir(), "changed.ts")
	deletedFile := filepath.Join(t.TempDir(), "deleted.ts")
	client.callback(nil)([]fswatch.Event{
		{Kind: fswatch.EventUpdate, Path: changedFile},
		{Kind: fswatch.EventDelete, Path: deletedFile},
	}, nil)

	summary, generation := client.drain()
	changedURI := lsconv.FileNameToDocumentURI(changedFile)
	deletedURI := lsconv.FileNameToDocumentURI(deletedFile)
	if generation != 1 {
		t.Fatalf("generation = %d, want 1", generation)
	}
	if !summary.Created.Has(changedURI) || !summary.Changed.Has(changedURI) {
		t.Fatal("updated path was not conservatively classified as created and changed")
	}
	if !summary.Deleted.Has(deletedURI) {
		t.Fatal("deleted path was not classified as deleted")
	}
	if !summary.IncludesWatchChangeOutsideNodeModules {
		t.Fatal("expected outside-node_modules change marker")
	}
	if !client.drainIsEmpty() {
		t.Fatal("drain did not clear pending changes")
	}
}

func TestLocalWatcherClientInvalidatesUpdateForCreateOnlyWatch(t *testing.T) {
	t.Parallel()

	client := &localWatcherClient{
		watches: make(map[project.WatcherID][]io.Closer),
		changes: make(map[string]pendingWatchChange),
	}
	fileName := filepath.Join(t.TempDir(), "changed.ts")
	kind := lsproto.WatchKindCreate
	client.callback(&kind)([]fswatch.Event{{Kind: fswatch.EventUpdate, Path: fileName}}, nil)

	summary, _ := client.drain()
	uri := lsconv.FileNameToDocumentURI(fileName)
	if !summary.Created.Has(uri) || !summary.Changed.Has(uri) {
		t.Fatal("create-only update was not classified as both created and changed")
	}
}

func TestLocalWatcherClientInvalidatesAfterTerminatedWatch(t *testing.T) {
	t.Parallel()

	client := &localWatcherClient{
		watches: make(map[project.WatcherID][]io.Closer),
		changes: make(map[string]pendingWatchChange),
	}
	client.callback(nil)(nil, fswatch.ErrWatchTerminated)
	for range 2 {
		summary, _ := client.drain()
		if !summary.InvalidateAll {
			t.Fatal("terminated watch did not preserve full invalidation")
		}
	}
}

func TestLocalWatcherClientInvalidatesAfterUnknownWatchError(t *testing.T) {
	t.Parallel()

	client := &localWatcherClient{
		watches: make(map[project.WatcherID][]io.Closer),
		changes: make(map[string]pendingWatchChange),
	}
	client.callback(nil)(nil, errors.New("watch failed"))
	summary, _ := client.drain()
	if !summary.InvalidateAll {
		t.Fatal("unknown watch error did not enable full invalidation")
	}
}

func (c *localWatcherClient) drainIsEmpty() bool {
	summary, _ := c.drain()
	return summary.IsEmpty()
}

type fakeCloser struct{ closed bool }

func (c *fakeCloser) Close() error {
	c.closed = true
	return nil
}

type fakeWatchBackend struct {
	directories []string
	files       []string
	closers     []*fakeCloser
}

func (b *fakeWatchBackend) WatchDirectory(dir string, _ fswatch.WatchCallback, _ ...fswatch.WatchOption) (io.Closer, error) {
	b.directories = append(b.directories, dir)
	closer := &fakeCloser{}
	b.closers = append(b.closers, closer)
	return closer, nil
}

func (b *fakeWatchBackend) WatchFile(path string, _ fswatch.WatchCallback) (io.Closer, error) {
	b.files = append(b.files, path)
	closer := &fakeCloser{}
	b.closers = append(b.closers, closer)
	return closer, nil
}

func TestLocalWatcherClientReplacesRegistration(t *testing.T) {
	t.Parallel()

	root := t.TempDir()
	backend := &fakeWatchBackend{}
	client := &localWatcherClient{
		backend: backend,
		watches: make(map[project.WatcherID][]io.Closer),
		changes: make(map[string]pendingWatchChange),
	}
	id := project.WatcherID("project")
	pattern := filepath.Join(root, "**", "*")
	watchers := []*lsproto.FileSystemWatcher{{GlobPattern: lsproto.PatternOrRelativePattern{Pattern: &pattern}}}
	if err := client.WatchFiles(context.Background(), id, watchers); err != nil {
		t.Fatal(err)
	}
	if err := client.WatchFiles(context.Background(), id, watchers); err != nil {
		t.Fatal(err)
	}
	if len(backend.closers) != 2 || !backend.closers[0].closed || backend.closers[1].closed {
		t.Fatal("replacing a registration did not close only the previous watch")
	}
	client.close()
	if !backend.closers[1].closed {
		t.Fatal("client close did not close active watch")
	}
}

func TestLocalWatcherClientRejectsRegistrationAfterClose(t *testing.T) {
	t.Parallel()

	backend := &fakeWatchBackend{}
	client := &localWatcherClient{
		backend: backend,
		watches: make(map[project.WatcherID][]io.Closer),
		changes: make(map[string]pendingWatchChange),
	}
	client.close()
	pattern := filepath.Join(t.TempDir(), "**", "*")
	err := client.WatchFiles(context.Background(), project.WatcherID("closed"), []*lsproto.FileSystemWatcher{{
		GlobPattern: lsproto.PatternOrRelativePattern{Pattern: &pattern},
	}})
	if err == nil {
		t.Fatal("closed client accepted a new registration")
	}
	if len(backend.closers) != 0 {
		t.Fatal("closed client opened a filesystem watch")
	}
}

func TestLocalWatcherClientIgnoresVirtualRegistration(t *testing.T) {
	t.Parallel()

	backend := &fakeWatchBackend{}
	client := &localWatcherClient{
		backend: backend,
		watches: make(map[project.WatcherID][]io.Closer),
		changes: make(map[string]pendingWatchChange),
	}
	pattern := "bundled:///libs/**/*"
	err := client.WatchFiles(context.Background(), project.WatcherID("virtual"), []*lsproto.FileSystemWatcher{{
		GlobPattern: lsproto.PatternOrRelativePattern{Pattern: &pattern},
	}})
	if err != nil {
		t.Fatal(err)
	}
	if len(backend.closers) != 0 {
		t.Fatal("virtual registration opened a filesystem watch")
	}
}
