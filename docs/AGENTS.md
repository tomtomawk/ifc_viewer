# Politique des formats et des migrations

Le logiciel est encore en développement et n'est pas en production. Ne pas ajouter de migration, de conversion automatique ni de chemin de compatibilité pour les anciennes versions des formats natifs uniquement pour préserver les anciens fichiers, sauf demande explicite de l'utilisateur. Les anciennes familles seront refaites.

Le JSON des familles doit décrire explicitement leur construction et celle de leurs éléments. Une forme doit notamment porter une opération de construction clairement identifiée et exclusive, avec les données nécessaires à sa définition. Privilégier un schéma cohérent, documenté et validé, et tester l'écriture, la lecture et le rejet des données invalides du schéma courant.

Cette règle ne supprime pas les fonctionnalités d'import et d'export de formats externes demandées par le projet. Elle ne justifie pas non plus la suppression de fichiers utilisateur. Signaler les changements incompatibles de format dans la documentation et dans le compte rendu des travaux.
