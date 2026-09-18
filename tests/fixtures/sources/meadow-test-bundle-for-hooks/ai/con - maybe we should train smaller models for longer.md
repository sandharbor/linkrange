[[T the chinchilla trap]]

Train [[dim - model size -- smaller]] [[dim - training time -- long]]

Basically, you should train the smallest model possible to achieve certain degree of loss. The reason is that inference costs on bigger models are quite large now. When running scale in production.

Apparently you can eek out a lot of performance from small models if you train them for a very long time.

[[con - smaller models cost less to run inference on]] [[inference might cost more than training]] [[training with inference efficiency as the goal]]