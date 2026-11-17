package genaicostprocessor // import "github.com/Mattbusel/llm-prices/otel/genaicostprocessor"

import (
	"bytes"
	"context"
	"crypto/ed25519"
	"encoding/base64"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"strings"
	"sync"
	"time"

	llmprices "github.com/Mattbusel/llm-prices/go"
	"go.uber.org/zap"

	"github.com/Mattbusel/llm-prices/otel/genaicostprocessor/internal/metadata"
)

// maxTableBytes bounds a downloaded table (LiteLLM's map is about 4 MB).
const maxTableBytes = 64 << 20

// reloader loads the price table from a URL or file, checks its signature, and installs it;
// with a reload interval it does so again in the background.
type reloader struct {
	cfg       *Config
	url       string
	key       ed25519.PublicKey
	logger    *zap.Logger
	install   func(priceSource)
	telemetry *metadata.TelemetryBuilder
	client    *http.Client

	mu        sync.Mutex
	version   string // version of the installed table
	generated string // generated date of the installed llm-prices table ("" when unknown)
	cancel    context.CancelFunc
	done      chan struct{}
}

func newReloader(cfg *Config, u string, logger *zap.Logger, install func(priceSource), tb *metadata.TelemetryBuilder) *reloader {
	r := &reloader{
		cfg:       cfg,
		url:       u,
		key:       cfg.publicKey(),
		logger:    logger,
		install:   install,
		telemetry: tb,
		client:    &http.Client{Timeout: cfg.DownloadTimeout},
	}
	if cfg.Source == SourceLLMPrices {
		// The embedded table serves until a load succeeds; nothing older replaces it.
		r.generated = llmprices.Default().Generated()
	}
	return r
}

// start loads the table once. When required is true a failed first load is an error; otherwise
// it is logged and the table already installed keeps serving. With a reload interval, start
// also launches the background reloads.
func (r *reloader) start(ctx context.Context, required bool) error {
	if err := r.reload(ctx); err != nil {
		if required {
			return fmt.Errorf("genaicost: loading prices from %s: %w", r.url, err)
		}
		r.logger.Warn("genaicost: could not load prices; using the table built into the processor",
			zap.String("url", r.url), zap.Error(err))
	}
	if r.cfg.ReloadInterval <= 0 {
		return nil
	}
	bg, cancel := context.WithCancel(context.Background())
	r.cancel, r.done = cancel, make(chan struct{})
	go r.loop(bg)
	return nil
}

func (r *reloader) loop(ctx context.Context) {
	defer close(r.done)
	t := time.NewTicker(r.cfg.ReloadInterval)
	defer t.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-t.C:
			if err := r.reload(ctx); err != nil && ctx.Err() == nil {
				r.logger.Warn("genaicost: price reload failed; keeping the current table",
					zap.String("url", r.url), zap.Error(err))
			}
		}
	}
}

func (r *reloader) stop() {
	if r.cancel != nil {
		r.cancel()
		<-r.done
	}
}

// reload fetches, verifies and parses the table, and installs it when its version differs from
// the one in use. A signed llm-prices table generated before the table in use (the embedded one
// included) is rejected: a signature covers only the table's bytes, so without this check an
// older signed release served at the URL could roll prices back.
func (r *reloader) reload(ctx context.Context) (err error) {
	defer func() {
		if err != nil {
			r.telemetry.ProcessorGenaicostPriceReloadFailures.Add(context.Background(), 1)
		}
	}()
	body, err := r.fetch(ctx, r.url)
	if err != nil {
		return err
	}
	if r.key != nil {
		sig, err := r.fetch(ctx, r.url+".sig")
		if err != nil {
			return fmt.Errorf("signature: %w", err)
		}
		if err := verify(r.key, body, sig); err != nil {
			return err
		}
	}
	src, err := parseSource(r.cfg.Source, body)
	if err != nil {
		return err
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	if src.pricingVersion() == r.version {
		return nil
	}
	generated := ""
	if lp, ok := src.(llmPricesSource); ok {
		generated = lp.table.Generated()
		if r.key != nil && r.generated != "" && generated < r.generated {
			return fmt.Errorf("table generated %q is older than the table in use (generated %q); not installed", generated, r.generated)
		}
	}
	r.install(src)
	r.version = src.pricingVersion()
	if generated != "" {
		r.generated = generated
	}
	r.telemetry.ProcessorGenaicostPriceReloads.Add(context.Background(), 1)
	r.logger.Info("genaicost: price table loaded",
		zap.String("url", r.url), zap.String("pricing.id", src.pricingID()), zap.String("pricing.version", src.pricingVersion()))
	return nil
}

// verify checks a detached Ed25519 signature: sig is the base64 signature of exactly body.
func verify(key ed25519.PublicKey, body, sig []byte) error {
	raw, err := base64.StdEncoding.DecodeString(strings.TrimSpace(string(sig)))
	if err != nil || len(raw) != ed25519.SignatureSize {
		return errors.New("signature is not a base64 Ed25519 signature")
	}
	if !ed25519.Verify(key, body, raw) {
		return errors.New("signature does not match the table")
	}
	return nil
}

// parseSource builds a price source of the given kind from a table's bytes.
func parseSource(kind string, body []byte) (priceSource, error) {
	switch kind {
	case SourceLLMPrices:
		t, err := llmprices.ParseTable(body)
		if err != nil {
			return nil, err
		}
		return llmPricesSource{table: t}, nil
	case SourceLiteLLM:
		return parseLiteLLM(body)
	}
	return nil, fmt.Errorf("unknown source %q", kind)
}

func (r *reloader) fetch(ctx context.Context, loc string) ([]byte, error) {
	u, err := url.Parse(loc)
	if err == nil && (u.Scheme == "http" || u.Scheme == "https") {
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, loc, nil)
		if err != nil {
			return nil, err
		}
		req.Header.Set("User-Agent", "genaicostprocessor (+https://github.com/Mattbusel/llm-prices)")
		resp, err := r.client.Do(req)
		if err != nil {
			return nil, err
		}
		defer resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("GET %s: %s", loc, resp.Status)
		}
		return readLimited(resp.Body)
	}
	path := loc
	if err == nil && u.Scheme == "file" {
		path = u.Path
		if u.Host != "" {
			path = u.Host + u.Path
		}
		// file:///C:/x on Windows parses to Path "/C:/x".
		if len(path) > 2 && path[0] == '/' && path[2] == ':' {
			path = path[1:]
		}
	}
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	return readLimited(f)
}

func readLimited(rd io.Reader) ([]byte, error) {
	b, err := io.ReadAll(io.LimitReader(rd, maxTableBytes+1))
	if err != nil {
		return nil, err
	}
	if len(b) > maxTableBytes {
		return nil, fmt.Errorf("table is larger than %d bytes", maxTableBytes)
	}
	return bytes.TrimPrefix(b, []byte("\xef\xbb\xbf")), nil
}
