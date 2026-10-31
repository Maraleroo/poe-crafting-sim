**Orb of Augmentation**
- Applies to: magic items.
- Not applicable when: the item is not magic. The reason shown is the game's message "Failed to apply item: Target is not Magic."
- Not applicable when: the item is magic and neither side has room under both affix limits. The reason shown is the game's message "Failed to apply item: Item has no space for more Mods."
- Effect: one random mod is added as described in "Adding a random mod". If the valid pool is empty, no mod is added, the item does not change and the reason shown is the game's message "Failed to apply item: Could not generate Mod."
- Evidence: tested by the PO on 2026-10-31. A normal item and a rare item: "Failed to apply item: Target is not Magic.". A magic item with 1 prefix and 1 suffix, a magic Composite Ring with 2 prefixes and 0 suffixes, and a magic Focused Amulet with 0 mods (limits 0 prefixes and 0 suffixes): "Failed to apply item: Item has no space for more Mods.". A magic Spectral Sword (item level 47) whose only mod is the crafted suffix "Cannot roll Attack Modifiers"; every prefix of this base up to item level 47 has the tag attack: "Failed to apply item: Could not generate Mod.".
