// Déclaration ambiante pour les CSS Modules (`*.module.css`), absente après la
// migration depuis Next.js (qui la fournissait via son propre plugin TypeScript).
declare module "*.module.css" {
	const classes: { readonly [className: string]: string };
	export default classes;
}
