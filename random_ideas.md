tether:
    - have tendrils appear on the contact point, spreading out from it (with some appropriate hooking sound, eg on impact + creaky (but short, hooking is a high frequency event, don't want to make this sound an annoyance))
    - subtle camera effect when the player is initially propelled?
enemies
    - the possiblity of "horror movement", where the creature temporarily glitches in and out of
    existence, with some clear visual effect (and sound, eg radio static) indicating the transition where it glitches out of point a
    and glitches into point b, with possibly some ghosting effect in the trajectory
    - the tether is somehow connected to enemies, eg the buds, and the player must harvest sufficient
    body parts to construct it
    - besides growling or attack sounds, there should also be footstep sounds to cement their presence (this is actually just an aspect of the overall sound (major) work)
beacon
    - use a duration + cooldown on an item (eg like a utility weapon), rather than a consumable.
    - the duration can be shown through diminishng power on the light beam
- input
    - use OS raw input, sub-frame polling, frame-pacing (" A known fix in the Bevy ecosystem is integrating a high-precision spin timer (like the spin_sleep crate) at the very end of the frame rendering pipeline. This delays the start of the next frame until the absolute last millisecond before the monitor's vertical blanking interval, ensuring that when the engine samples your mouse data") to eliminate mouse input lag 
- world
    - try to always have a player-scale object in the foreground when megastructures are present, they serve to ensure the effect works by having a comparison reference object. One way to do this is to have recognizable smaller scale structures surrounding the megastructure, or at least visible in-frame. Another is to include said type of objects
    which the player can recognize as inhabitants of the megastructure, they would read as small details in that context, but still visible for the comparison. known enemies on the megastructure can also serve that purpose.