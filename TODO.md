# TODO

- [ ] Confirm on hardware that the root partition expanded to fill the card on
      first boot. Everything else about the shortened image is verified: it
      boots, the receiver links to the goggles at boot with the cable already
      attached, and an update installs on top of it. One command answers it:
      `ssh root@192.168.50.1 'df -h /'` — expect ~6.7 G on the 8 GB card, not
      2.5 G.
