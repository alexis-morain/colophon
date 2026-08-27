# Prompt de session en cours

état : aucun

<!--
Ce fichier est la seule surface de passation entre Alexis et une session cloud.
La routine nocturne le lit et n'agit QUE si la première ligne d'état dit
« à exécuter ». Dans tout autre cas elle s'arrête sans rien faire et sans
ouvrir de PR.

Trois états, et rien d'autre :

  état : aucun            aucune session en attente, la routine ne fait rien
  état : à exécuter       la routine prend le prompt ci-dessous
  état : consommé le AAAA-MM-JJ par #<numéro de PR>

La session qui exécute le prompt bascule l'état à « consommé » dans la PR
qu'elle ouvre, jamais avant. Une PR non fusionnée laisse donc le fichier
consommé : c'est voulu, une seule tentative par prompt, on ne repasse pas
dessus sans qu'Alexis ait relu.

Le prompt lui-même s'écrit comme les prompts de session du dossier de gestion
(`plans/*-prompt-session-*.md`) : les arbitrages sont pris d'avance, la session
n'a plus qu'à coder. Un prompt qui laisse un arbitrage ouvert n'est pas prêt.

Ce qu'un prompt doit toujours contenir :
  - ce que la session livre, en une phrase
  - les décisions déjà prises, numérotées
  - ce qu'elle ne touche pas
  - comment on saura que c'est fait
-->

## Le prompt

(vide)
