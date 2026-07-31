# Migrate CI to Blacksmith runners

## Summary

Our GitHub Actions builds are slow on the default runners. Move the main workflows to Blacksmith runners.

## Details

- Swap `runs-on: ubuntu-latest` for the Blacksmith equivalent
- Verify cache hit rates after the switch
- Compare build times for a week before removing the old config
