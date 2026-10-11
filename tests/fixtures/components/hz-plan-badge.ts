// Pastille d'offre, écrite à la main sans décorateur de classe.
import {LitElement, html, css} from "lit"
import {property, state} from "lit/decorators.js"

/* Les tailles suivent l'échelle du kit. */
export class HzPlanBadge extends LitElement {
	static styles = css`
		:host { display: inline-flex; }
	`

	// Taille de la pastille.
	@property({reflect: true})
	size: 'sm' | 'md' | 'lg' = 'md'

	@property() label = ''
	@property({type: Number}) count = 0
	@state()   private open = false
	@property() note?: string

	render() {
		return html`<span class="badge  badge--round" role="status">
			<strong>${this.label}</strong>
			Encore ${this.count} places
			<button type="button" disabled>Choisir</button>
		</span>`
	}
}

customElements.define("hz-plan-badge", HzPlanBadge)
