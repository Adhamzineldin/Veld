package output

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// Options control how a session's staged files are planned.
type Options struct {
	VeldVersion string
	Emitters    []string // recorded in the manifest
	// OverwriteUnmanaged allows replacing files that exist in an output
	// directory with different content but are not listed in its manifest.
	OverwriteUnmanaged bool
	// Partial keeps managed files this run did not produce (incremental
	// builds emit only some modules), so nothing is deleted.
	Partial bool
}

// Plan is the set of changes Apply will make. Building it reads the output
// directories but never modifies them.
type Plan struct {
	Dirs []DirPlan
}

// DirPlan is the plan for one output directory. Paths are slash-separated
// and relative to Dir; every list is sorted.
type DirPlan struct {
	Dir       string
	Create    []string // new files
	Update    []string // files whose content changes
	Unchanged []string // files left untouched (mtime preserved)
	Delete    []string // managed files no longer generated
	Keep      []string // stale managed files edited since generation; left in place, no longer managed

	staged map[string]stagedFile
	old    Manifest
	next   Manifest
}

type stagedFile struct {
	data []byte
	perm os.FileMode
	sum  string
}

// ConflictError reports files that generation would overwrite although Veld
// did not write them.
type ConflictError struct {
	Paths []string // absolute paths, each with the reason in parentheses
}

func (e *ConflictError) Error() string {
	return fmt.Sprintf("%d file(s) in the output directory were not written by Veld and would be overwritten:\n  %s",
		len(e.Paths), strings.Join(e.Paths, "\n  "))
}

// Plan compares every staged directory with its real output directory and
// previous manifest. It returns a *ConflictError, and plans nothing, if a
// staged file would overwrite an unmanaged file.
func (s *Session) Plan(o Options) (*Plan, error) {
	plan := &Plan{}
	var conflicts []string
	for _, d := range s.dirs {
		dp, c, err := planDir(d, o)
		if err != nil {
			return nil, err
		}
		conflicts = append(conflicts, c...)
		plan.Dirs = append(plan.Dirs, dp)
	}
	if len(conflicts) > 0 {
		return nil, &ConflictError{Paths: conflicts}
	}
	return plan, nil
}

func planDir(d stagedDir, o Options) (DirPlan, []string, error) {
	old, _, err := ReadManifest(d.real)
	if err != nil {
		return DirPlan{}, nil, err
	}
	files, err := readStaged(d.staged)
	if err != nil {
		return DirPlan{}, nil, err
	}
	managed := old.index()
	dp := DirPlan{Dir: d.real, staged: files, old: old}

	var conflicts []string
	for _, rel := range sortedKeys(files) {
		dest := filepath.Join(d.real, filepath.FromSlash(rel))
		info, err := os.Lstat(dest)
		switch {
		case errors.Is(err, fs.ErrNotExist):
			dp.Create = append(dp.Create, rel)
			continue
		case err != nil:
			return DirPlan{}, nil, fmt.Errorf("inspect %s: %w", dest, err)
		case !info.Mode().IsRegular():
			conflicts = append(conflicts, dest+" (not a regular file)")
			continue
		}
		sum, _, err := hashFile(dest)
		if err != nil {
			return DirPlan{}, nil, fmt.Errorf("hash %s: %w", dest, err)
		}
		_, isManaged := managed[rel]
		switch {
		case sum == files[rel].sum:
			dp.Unchanged = append(dp.Unchanged, rel)
		case isManaged || o.OverwriteUnmanaged:
			dp.Update = append(dp.Update, rel)
		default:
			conflicts = append(conflicts, dest+" (not in "+ManifestName+")")
		}
	}

	if !o.Partial {
		for _, e := range old.Files {
			if _, still := files[e.Path]; still {
				continue
			}
			sum, isFile, err := hashFile(filepath.Join(d.real, filepath.FromSlash(e.Path)))
			if err != nil {
				return DirPlan{}, nil, fmt.Errorf("hash %s: %w", e.Path, err)
			}
			switch {
			case !isFile:
				// Already gone (or replaced by a non-file): just stop tracking it.
			case sum == e.SHA256:
				dp.Delete = append(dp.Delete, e.Path)
			default:
				dp.Keep = append(dp.Keep, e.Path)
			}
		}
		sort.Strings(dp.Delete)
		sort.Strings(dp.Keep)
	}

	dp.next = nextManifest(old, files, o)
	return dp, conflicts, nil
}

func nextManifest(old Manifest, files map[string]stagedFile, o Options) Manifest {
	next := Manifest{VeldVersion: o.VeldVersion, Emitters: append([]string(nil), o.Emitters...)}
	for rel, f := range files {
		next.Files = append(next.Files, Entry{Path: rel, SHA256: f.sum})
	}
	if o.Partial {
		next.Emitters = append(next.Emitters, old.Emitters...)
		for _, e := range old.Files {
			if _, replaced := files[e.Path]; !replaced {
				next.Files = append(next.Files, e)
			}
		}
	}
	return next
}

// readStaged loads every file an emitter wrote into the staging directory.
func readStaged(root string) (map[string]stagedFile, error) {
	files := map[string]stagedFile{}
	err := filepath.WalkDir(root, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			return nil
		}
		rel, err := filepath.Rel(root, p)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		if !d.Type().IsRegular() {
			return fmt.Errorf("emitter produced %s, which is not a regular file", rel)
		}
		if err := validRelPath(rel); err != nil {
			return fmt.Errorf("emitter produced an invalid file: %w", err)
		}
		info, err := d.Info()
		if err != nil {
			return err
		}
		data, err := os.ReadFile(p)
		if err != nil {
			return err
		}
		files[rel] = stagedFile{data: data, perm: info.Mode().Perm(), sum: hashBytes(data)}
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("read staged output: %w", err)
	}
	return files, nil
}

// Changed reports whether Apply would modify anything on disk.
func (p *Plan) Changed() bool {
	for _, d := range p.Dirs {
		if len(d.Create)+len(d.Update)+len(d.Delete) > 0 {
			return true
		}
	}
	return false
}

// Apply writes the plan: new and changed files are written atomically, then
// stale managed files are deleted, then the manifest is written. If a step
// fails, the manifest still records every file Veld has written, so a later
// run or `veld clean` can manage it.
func (p *Plan) Apply() error {
	for i := range p.Dirs {
		if err := p.Dirs[i].apply(); err != nil {
			return err
		}
	}
	return nil
}

func (dp *DirPlan) apply() error {
	if err := os.MkdirAll(dp.Dir, 0o755); err != nil {
		return fmt.Errorf("create output directory %s: %w", dp.Dir, err)
	}
	written := map[string]string{}
	err := func() error {
		for _, rel := range append(append([]string(nil), dp.Create...), dp.Update...) {
			f := dp.staged[rel]
			if err := writeAtomic(filepath.Join(dp.Dir, filepath.FromSlash(rel)), f.data, f.perm); err != nil {
				return err
			}
			written[rel] = f.sum
		}
		for _, rel := range dp.Delete {
			dest := filepath.Join(dp.Dir, filepath.FromSlash(rel))
			if err := os.Remove(dest); err != nil && !errors.Is(err, fs.ErrNotExist) {
				return fmt.Errorf("remove stale file %s: %w", dest, err)
			}
			removeEmptyParents(dp.Dir, dest)
		}
		return nil
	}()
	if err != nil {
		if merr := writeManifest(dp.Dir, recoveryManifest(dp.old, written)); merr != nil {
			return errors.Join(err, merr)
		}
		return err
	}
	return writeManifest(dp.Dir, dp.next)
}

// recoveryManifest lists the old managed files plus those written before a failure.
func recoveryManifest(old Manifest, written map[string]string) Manifest {
	rec := Manifest{VeldVersion: old.VeldVersion, Emitters: old.Emitters}
	for _, e := range old.Files {
		if _, ok := written[e.Path]; !ok {
			rec.Files = append(rec.Files, e)
		}
	}
	for rel, sum := range written {
		rec.Files = append(rec.Files, Entry{Path: rel, SHA256: sum})
	}
	return rec
}

func sortedKeys(m map[string]stagedFile) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}
