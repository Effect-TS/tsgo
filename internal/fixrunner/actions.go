// Package fixrunner executes registered Effect code-fix providers.
package fixrunner

import (
	"context"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/fixables"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/ls"
)

// CollectActions runs every registered provider for a diagnostic code.
func CollectActions(
	ctx context.Context,
	fixCtx *ls.CodeFixContext,
	options *etscore.ResolvedEffectPluginOptions,
	ch *checker.Checker,
	tp *typeparser.TypeParser,
	includeDisable bool,
) []*ls.CodeAction {
	fCtx := fixable.NewContext(ctx, fixCtx, options, ch, tp)
	var actions []*ls.CodeAction
	for _, provider := range fixables.ByErrorCode(fixCtx.ErrorCode) {
		if !includeDisable && provider.Name == fixables.EffectDisable.Name {
			continue
		}
		for _, result := range provider.Run(fCtx) {
			action := result
			actions = append(actions, &action)
		}
	}
	return actions
}
