from [[V how a transformer is used at inference time versus training time]]... you need only a single [[forward pass]] for [[training]] on a sequence to sequence comparison.  During [[inference]], however, you need a forward pass per [[token]]

[[con - the KV Cache is only used during inference because it is done serially]]

[[inference might cost more than training]]

[[con - inference can be cheaper with one bit models]]

