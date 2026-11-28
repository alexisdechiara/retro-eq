# Vérifier la compensation du volume sur ce qui joue

Status: ready-for-agent
Type: task

PRODUCT.md : « compensation calculée et vérifiée sur ce qui joue ». ADR-0006 : la capture sert « à afficher l'écart mesuré ».

Aujourd'hui le spectre capté ne sert qu'au calcul. Cumuler le volume perçu mesuré pendant les passages de A et de B d'un duel, en déduire l'écart mesuré en dB et le garder avec le duel dans le journal. L'afficher sur l'écran Résultat et signaler un duel dont l'écart dépasse un seuil. Testable avec la capture simulée ; la validation réelle demande le PC (ticket 16).
