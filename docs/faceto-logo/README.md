# faceto — logo

Système : wordmark **facet + octogone** (l'octogone remplace le « o »).
Encre neutre par défaut ; l'octogone est le *slot d'accent* — une couleur par atelier.
Fonte du wordmark vectorisée (Space Grotesk Bold, tracés) → aucune dépendance à la fonte installée.

## Fichiers

| Fichier | Usage |
| --- | --- |
| `faceto-wordmark.svg` | Lockup principal (en-tête site, bannière README) |
| `faceto-icon.svg` | Icône carrée — avatar, en-tête |
| `faceto-favicon.svg` | Favicon anneau (net ≥ ~20 px) |
| `faceto-favicon-16.svg` | Favicon plein (repli 16 px) |
| `faceto-wordmark-eventstorming.svg` | Exemple accent orange |
| `faceto-wordmark-{ink,white}.svg` | Bannière README, `<picture>` clair / sombre (octogone orange, texte encre / blanc) |
| `faceto-avatar-512-*.png` | Avatar GitHub 512 px (ink / white) |
| `faceto-social-card.{svg,png}` | Carte social preview GitHub 1280×640 (Open Graph) — `.png` à téléverser, `.svg` = source |

## Couleur

Tout est en `currentColor` → piloté par la propriété CSS `color`, dark mode automatique.

Accent par atelier (recoloriser uniquement l'octogone) :

```css
/* SVG inliné dans le HTML */
.faceto-o { stroke: var(--faceto-accent, currentColor); }
```

| Atelier | Token |
| --- | --- |
| Event Storming | `#FF9F1C` |
| C4 (futur) | `#2D6CDF` |
| Atelier N… | `#1D9E75` |

> L'accent Event Storming est aligné sur le token `lane-event` de `DESIGN.md` (`#FF9F1C`) :
> un seul orange dans tout le produit. Le favicon in-app (`src/template.html`) réutilise
> cette même valeur.

Pour un favicon plein coloré : remplacer `stroke` par `fill` dans le sélecteur.
